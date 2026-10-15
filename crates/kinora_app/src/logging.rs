//! Logging: `tracing` to stderr and to a daily-rotated file in the OS log folder (brief §8.6).
//! Panics are logged too, so a crash report is always in the log file.

use std::path::Path;

use anyhow::Context as _;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer as _;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

/// Keeps the background log writer alive; dropping it flushes the file.
#[derive(Debug)]
pub struct LogGuard {
    _file: Option<WorkerGuard>,
}

/// Starts logging. The level comes from `RUST_LOG` (e.g. `RUST_LOG=debug`), default `info`.
/// If the log folder can't be created, logging continues on stderr only.
pub fn init(log_dir: Option<&Path>) -> anyhow::Result<LogGuard> {
    let filter = || {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info,wgpu_core=warn,wgpu_hal=warn,naga=warn"))
    };
    let stderr = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_filter(filter());

    let (file_layer, guard) = match log_dir.map(file_writer) {
        Some(Ok((writer, guard))) => (
            Some(
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_writer(writer)
                    .with_filter(filter()),
            ),
            Some(guard),
        ),
        _ => (None, None),
    };

    tracing_subscriber::registry()
        .with(stderr)
        .with(file_layer)
        .try_init()
        .map_err(|err| anyhow::anyhow!(err))
        .context("could not initialise logging")?;

    if let Some(dir) = log_dir {
        tracing::debug!(dir = %dir.display(), "log folder");
    }
    install_panic_hook();
    Ok(LogGuard { _file: guard })
}

fn file_writer(
    dir: &Path,
) -> anyhow::Result<(tracing_appender::non_blocking::NonBlocking, WorkerGuard)> {
    std::fs::create_dir_all(dir)?;
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("kinora")
        .filename_suffix("log")
        .max_log_files(7)
        .build(dir)?;
    Ok(tracing_appender::non_blocking(appender))
}

/// Routes panic messages into the log (the default hook only prints to stderr, which a
/// GUI app on Windows doesn't have).
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        tracing::error!(thread = thread.name().unwrap_or("unnamed"), "panic: {info}");
        default_hook(info);
    }));
}
