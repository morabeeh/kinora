//! Command-line arguments. File managers start us as `kinora <file>`; the options are for
//! testing and troubleshooting.

use std::ffi::OsString;
use std::path::PathBuf;

use kinora_core::settings::RendererChoice;

pub const USAGE: &str = "\
Usage: kinora [options] [file]

Options:
  --lite                 force Lite mode (fewer threads, smaller caches, no effects)
  --renderer <name>      auto | hardware | software (also: SLINT_BACKEND=winit-software)
  --new-window           don't hand the file to an already running Kinora
  --bench-startup        log startup timings and quit once the first picture is shown
  --version              print the version
  -h, --help             print this help

Environment: KINORA_SIMULATE=lite simulates a Lite-tier PC (2 cores, 2 GB, no GPU);
RUST_LOG=debug shows more log detail.";

/// What the command line asks for.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Start the viewer (default).
    #[default]
    Run,
    /// Print the usage text.
    Help,
    /// Print the version.
    Version,
}

/// Parsed command line.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Args {
    pub action: Action,
    pub files: Vec<PathBuf>,
    pub lite: bool,
    pub renderer: Option<RendererChoice>,
    pub new_window: bool,
    pub bench_startup: bool,
}

/// Parses the arguments after the program name.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Args, String> {
    let mut parsed = Args::default();
    let mut args = args.into_iter();
    let mut only_files = false;
    while let Some(arg) = args.next() {
        let text = arg.to_string_lossy();
        if only_files || !text.starts_with('-') || text == "-" {
            parsed.files.push(PathBuf::from(arg));
            continue;
        }
        match text.as_ref() {
            "--" => only_files = true,
            "--lite" => parsed.lite = true,
            "--new-window" => parsed.new_window = true,
            "--bench-startup" => parsed.bench_startup = true,
            "-h" | "--help" | "/?" => parsed.action = Action::Help,
            "--version" => parsed.action = Action::Version,
            "--renderer" => {
                let value = args.next().ok_or("--renderer needs a value")?;
                parsed.renderer = Some(renderer(&value.to_string_lossy())?);
            }
            other => match other.strip_prefix("--renderer=") {
                Some(value) => parsed.renderer = Some(renderer(value)?),
                None => return Err(format!("unknown option `{other}`")),
            },
        }
    }
    Ok(parsed)
}

fn renderer(value: &str) -> Result<RendererChoice, String> {
    match value.to_ascii_lowercase().as_str() {
        "auto" => Ok(RendererChoice::Auto),
        "hardware" | "gpu" => Ok(RendererChoice::Hardware),
        "software" | "cpu" => Ok(RendererChoice::Software),
        other => Err(format!(
            "unknown renderer `{other}` (use auto, hardware or software)"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strs(args: &[&str]) -> Result<Args, String> {
        parse(args.iter().map(OsString::from))
    }

    #[test]
    fn file_manager_style() {
        let args = parse_strs(&["C:\\Photos\\a b.jpg"]).unwrap();
        assert_eq!(args.files, [PathBuf::from("C:\\Photos\\a b.jpg")]);
        assert!(!args.lite && args.renderer.is_none());
    }

    #[test]
    fn options() {
        let args =
            parse_strs(&["--lite", "--renderer", "software", "x.png", "--new-window"]).unwrap();
        assert!(args.lite && args.new_window);
        assert_eq!(args.renderer, Some(RendererChoice::Software));
        assert_eq!(args.files, [PathBuf::from("x.png")]);
        let args = parse_strs(&["--renderer=GPU", "--bench-startup"]).unwrap();
        assert_eq!(args.renderer, Some(RendererChoice::Hardware));
        assert!(args.bench_startup);
    }

    #[test]
    fn double_dash_ends_options() {
        let args = parse_strs(&["--", "--lite"]).unwrap();
        assert_eq!(args.files, [PathBuf::from("--lite")]);
        assert!(!args.lite);
    }

    #[test]
    fn errors() {
        assert!(parse_strs(&["--renderer"]).is_err());
        assert!(parse_strs(&["--renderer", "vulkan"]).is_err());
        assert!(parse_strs(&["--frobnicate"]).is_err());
        assert_eq!(parse_strs(&["-h"]).unwrap().action, Action::Help);
    }
}
