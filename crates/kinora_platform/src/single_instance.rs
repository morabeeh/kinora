//! Single-instance mode (brief §5.1): opening a file while Kinora already runs hands the
//! file to the running window instead of opening a second one, like Windows Photos.
//!
//! The first Kinora becomes the **primary** and listens on a per-user local socket (a named
//! pipe on Windows, a Unix socket in `$XDG_RUNTIME_DIR` on Linux). A later Kinora connects,
//! sends the file paths, waits for a one-byte acknowledgement and exits.
//!
//! Wire format (version 1), all integers little-endian:
//! `"KINORA/1\n"`, then per path a `u32` byte length and the path's native bytes (UTF-8-ish
//! bytes on Unix, UTF-16 code units on Windows), then a `u32` zero. The reply is one byte.

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::thread::JoinHandle;
use std::time::Duration;

use interprocess::ConnectWaitMode;
use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{ConnectOptions, ListenerOptions, Name};

const MAGIC: &[u8] = b"KINORA/1\n";
const ACK: u8 = 0x06;
/// Limits so a confused or hostile client can't make us allocate a lot.
const MAX_PATHS: usize = 256;
const MAX_PATH_BYTES: u32 = 64 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(2);

/// Result of [`start`].
#[derive(Debug)]
pub enum Startup {
    /// We are the first Kinora: keep running and serve requests with [`Primary::serve`].
    Primary(Primary),
    /// Another Kinora took our files; this process should exit.
    Forwarded,
}

/// The listening side, owned by the first Kinora process.
pub struct Primary {
    listener: LocalSocketListener,
}

impl std::fmt::Debug for Primary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Primary").finish_non_exhaustive()
    }
}

/// Files another Kinora asked us to show (may be empty: "just bring the window forward").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRequest {
    /// Paths as given to the other process (already made absolute by it).
    pub paths: Vec<PathBuf>,
}

/// Connects to a running Kinora and forwards `paths`, or becomes the primary instance.
///
/// # Errors
/// If neither connecting nor listening works (e.g. no runtime directory). The caller should
/// then simply run as a normal, independent window.
pub fn start(paths: &[PathBuf]) -> io::Result<Startup> {
    start_named(&socket_name()?, paths)
}

fn start_named(name: &Name<'static>, paths: &[PathBuf]) -> io::Result<Startup> {
    if forward(name, paths).is_ok() {
        return Ok(Startup::Forwarded);
    }
    match ListenerOptions::new()
        .name(name.clone())
        // Unix only: replace a socket file left behind by a crashed Kinora. A live primary
        // would have accepted our connection above.
        .try_overwrite(true)
        .create_sync()
    {
        Ok(listener) => Ok(Startup::Primary(Primary { listener })),
        Err(listen_error) => {
            // Two Kinoras starting at the same moment: the other one won the race.
            std::thread::sleep(Duration::from_millis(100));
            forward(name, paths)
                .map(|()| Startup::Forwarded)
                .map_err(|_| listen_error)
        }
    }
}

impl Primary {
    /// Serves requests on a background thread for the rest of the process's life, calling
    /// `on_request` for each one (on that thread — hop to the UI thread inside it).
    ///
    /// # Errors
    /// If the thread can't be spawned.
    pub fn serve(
        self,
        on_request: impl Fn(OpenRequest) + Send + Sync + 'static,
    ) -> io::Result<JoinHandle<()>> {
        let on_request = std::sync::Arc::new(on_request);
        std::thread::Builder::new()
            .name(String::from("single-instance"))
            .spawn(move || {
                for connection in self.listener.incoming() {
                    let stream = match connection {
                        Ok(stream) => stream,
                        Err(err) => {
                            tracing::warn!(%err, "single-instance connection failed");
                            continue;
                        }
                    };
                    // One short-lived thread per client, so a client that connects and then
                    // stalls can't block the others.
                    let on_request = std::sync::Arc::clone(&on_request);
                    let spawned = std::thread::Builder::new()
                        .name(String::from("single-instance-client"))
                        .spawn(move || match receive(stream) {
                            Ok(request) => on_request(request),
                            Err(err) => {
                                tracing::warn!(%err, "ignored a bad single-instance request");
                            }
                        });
                    if let Err(err) = spawned {
                        tracing::warn!(%err, "could not handle a single-instance request");
                    }
                }
            })
    }
}

/// Sends `paths` to the primary instance and waits for its acknowledgement.
///
/// Named pipes on Windows have no I/O timeouts, so the exchange runs on a helper thread and
/// we stop waiting after [`IO_TIMEOUT`]: a hung primary must not hang this process too.
fn forward(name: &Name<'static>, paths: &[PathBuf]) -> io::Result<()> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let name = name.clone();
    let paths = paths.to_vec();
    std::thread::Builder::new()
        .name(String::from("single-instance-client"))
        .spawn(move || {
            // The receiver may be gone after a timeout; nothing left to report to then.
            let _ = sender.send(forward_blocking(&name, &paths));
        })?;
    receiver.recv_timeout(IO_TIMEOUT).unwrap_or_else(|_| {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "the running Kinora did not answer",
        ))
    })
}

fn forward_blocking(name: &Name<'static>, paths: &[PathBuf]) -> io::Result<()> {
    let mut stream = ConnectOptions::new()
        .name(name.clone())
        .wait_mode(ConnectWaitMode::Timeout(Duration::from_millis(500)))
        .connect_sync()?;
    set_timeouts(&stream)?;
    #[cfg(windows)]
    crate::sys_windows::allow_foreground_activation();
    stream.write_all(&encode(paths))?;
    stream.flush()?;
    let mut ack = [0u8; 1];
    stream.read_exact(&mut ack)?;
    if ack == [ACK] {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected reply",
        ))
    }
}

fn receive(mut stream: LocalSocketStream) -> io::Result<OpenRequest> {
    set_timeouts(&stream)?;
    let request = decode(&mut stream)?;
    stream.write_all(&[ACK])?;
    stream.flush()?;
    Ok(request)
}

/// Unix sockets get I/O timeouts; Windows named pipes don't support them (the client side
/// uses a helper thread instead, and the server handles each client on its own thread).
fn set_timeouts(stream: &LocalSocketStream) -> io::Result<()> {
    for result in [
        stream.set_recv_timeout(Some(IO_TIMEOUT)),
        stream.set_send_timeout(Some(IO_TIMEOUT)),
    ] {
        match result {
            Err(err) if err.kind() != io::ErrorKind::Unsupported => return Err(err),
            _ => {}
        }
    }
    Ok(())
}

fn encode(paths: &[PathBuf]) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    for path in paths.iter().take(MAX_PATHS) {
        let bytes = native_bytes(path);
        let Ok(len) = u32::try_from(bytes.len()) else {
            continue;
        };
        if len == 0 || len > MAX_PATH_BYTES {
            continue;
        }
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&bytes);
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    out
}

fn decode(reader: &mut impl Read) -> io::Result<OpenRequest> {
    let invalid = |what: &str| io::Error::new(io::ErrorKind::InvalidData, what.to_owned());
    let mut magic = [0u8; MAGIC.len()];
    reader.read_exact(&mut magic)?;
    if magic != MAGIC {
        return Err(invalid("not a Kinora request"));
    }
    let mut paths = Vec::new();
    loop {
        let mut len = [0u8; 4];
        reader.read_exact(&mut len)?;
        let len = u32::from_le_bytes(len);
        if len == 0 {
            return Ok(OpenRequest { paths });
        }
        if len > MAX_PATH_BYTES || paths.len() >= MAX_PATHS {
            return Err(invalid("request too large"));
        }
        let mut bytes = vec![0u8; usize::try_from(len).map_err(|_| invalid("length"))?];
        reader.read_exact(&mut bytes)?;
        paths.push(path_from_native(&bytes).ok_or_else(|| invalid("bad path encoding"))?);
    }
}

#[cfg(unix)]
fn native_bytes(path: &std::path::Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt as _;
    path.as_os_str().as_bytes().to_vec()
}

/// Rebuilds a path sent by another Kinora. `None` for bytes no real path can have (a NUL
/// character), so a garbled request is rejected instead of opening a strange path.
#[cfg(unix)]
fn path_from_native(bytes: &[u8]) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt as _;
    if bytes.contains(&0) {
        return None;
    }
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
}

#[cfg(windows)]
fn native_bytes(path: &std::path::Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt as _;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(windows)]
fn path_from_native(bytes: &[u8]) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt as _;
    let (pairs, rest) = bytes.as_chunks::<2>();
    if !rest.is_empty() {
        return None;
    }
    let wide: Vec<u16> = pairs.iter().map(|pair| u16::from_le_bytes(*pair)).collect();
    if wide.contains(&0) {
        return None;
    }
    Some(PathBuf::from(std::ffi::OsString::from_wide(&wide)))
}

/// A socket name private to this user (and, on Windows, this login session).
fn socket_name() -> io::Result<Name<'static>> {
    #[cfg(windows)]
    {
        use interprocess::local_socket::GenericNamespaced;
        let user = std::env::var("USERNAME").unwrap_or_default();
        let domain = std::env::var("USERDOMAIN").unwrap_or_default();
        let session = crate::sys_windows::session_id().unwrap_or(0);
        format!("kinora-{}-{session}", sanitize(&format!("{domain}-{user}")))
            .to_ns_name::<GenericNamespaced>()
    }
    #[cfg(target_os = "linux")]
    {
        use interprocess::local_socket::{GenericFilePath, GenericNamespaced};
        // $XDG_RUNTIME_DIR is private to the user (mode 0700), so nobody else can connect.
        match std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from) {
            Some(dir) if dir.is_dir() => dir.join("kinora.sock").to_fs_name::<GenericFilePath>(),
            _ => {
                let user = std::env::var("USER").unwrap_or_default();
                format!("kinora-{}", sanitize(&user)).to_ns_name::<GenericNamespaced>()
            }
        }
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        use interprocess::local_socket::GenericNamespaced;
        let user = std::env::var("USER").unwrap_or_default();
        format!("kinora-{}", sanitize(&user)).to_ns_name::<GenericNamespaced>()
    }
}

/// Keeps only characters that are safe in pipe and socket names.
fn sanitize(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_round_trip() {
        let paths = vec![
            PathBuf::from("/photos/Ölbild 1.jpg"),
            PathBuf::from("C:\\Users\\me\\写真.heic"),
        ];
        let bytes = encode(&paths);
        let decoded = decode(&mut bytes.as_slice()).unwrap();
        assert_eq!(decoded.paths, paths);
        let empty = decode(&mut encode(&[]).as_slice()).unwrap();
        assert_eq!(empty.paths, Vec::<PathBuf>::new());
    }

    #[test]
    fn bad_requests_are_rejected() {
        assert!(decode(&mut b"HELLO/1\n\0\0\0\0".as_slice()).is_err());
        let mut huge = MAGIC.to_vec();
        huge.extend_from_slice(&(MAX_PATH_BYTES + 1).to_le_bytes());
        assert!(decode(&mut huge.as_slice()).is_err());
        let mut truncated = MAGIC.to_vec();
        truncated.extend_from_slice(&10u32.to_le_bytes());
        truncated.extend_from_slice(b"abc");
        assert!(decode(&mut truncated.as_slice()).is_err());
    }

    #[test]
    fn paths_with_nul_characters_are_rejected() {
        let mut request = MAGIC.to_vec();
        let garbled = native_bytes(std::path::Path::new("a\0b.jpg"));
        request.extend_from_slice(&u32::try_from(garbled.len()).unwrap().to_le_bytes());
        request.extend_from_slice(&garbled);
        request.extend_from_slice(&0u32.to_le_bytes());
        assert!(decode(&mut request.as_slice()).is_err());
    }

    #[test]
    fn second_instance_forwards_to_the_first() {
        use interprocess::local_socket::GenericNamespaced;
        let name = format!("kinora-test-{}", std::process::id())
            .to_ns_name::<GenericNamespaced>()
            .unwrap();
        let Startup::Primary(primary) = start_named(&name, &[]).unwrap() else {
            panic!("the first start must become the primary");
        };
        let (sender, receiver) = std::sync::mpsc::channel();
        let _server = primary
            .serve(move |request| sender.send(request).unwrap())
            .unwrap();
        let file = PathBuf::from("/photos/second.jpg");
        let second = start_named(&name, std::slice::from_ref(&file)).unwrap();
        assert!(matches!(second, Startup::Forwarded));
        let request = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(request.paths, [file]);
    }

    #[test]
    fn names_are_sanitized() {
        assert_eq!(sanitize("CORP\\Jo Doe"), "CORP_Jo_Doe");
        assert_eq!(sanitize("a-b_c9"), "a-b_c9");
    }
}
