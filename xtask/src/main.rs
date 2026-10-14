//! Developer task runner, invoked as `cargo xtask <command>` (alias in `.cargo/config.toml`).
//!
//! Think of it as the project's build script written in Rust instead of PowerShell/Bash,
//! so the same commands work on Linux, Windows and macOS.

// xtask is a command-line developer tool that is never shipped to users. Printing to the
// terminal *is* its user interface, so the workspace ban on print macros does not apply here.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::process::{Command, ExitCode};

const HELP: &str = "\
Usage: cargo xtask <command>

Commands:
  check-all   fmt --check, clippy (-D warnings), tests, cargo deny — stops at the first failure
  licenses    regenerate THIRD_PARTY_NOTICES.md with cargo-about
  help        show this message";

fn main() -> ExitCode {
    let command = std::env::args().nth(1);
    let result = match command.as_deref() {
        Some("check-all") => check_all(),
        Some("licenses") => licenses(),
        Some("help" | "-h" | "--help") | None => {
            println!("{HELP}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command `{other}`\n\n{HELP}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}

fn check_all() -> Result<(), String> {
    cargo(&["fmt", "--all", "--check"])?;
    cargo(&[
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ])?;
    cargo(&["test", "--workspace"])?;
    cargo(&["deny", "check"])?;
    println!("\nxtask: all checks passed ✔");
    Ok(())
}

fn licenses() -> Result<(), String> {
    cargo(&[
        "about",
        "generate",
        "--workspace",
        "about.hbs",
        "--output-file",
        "THIRD_PARTY_NOTICES.md",
    ])
}

/// Runs `cargo <args>` in the workspace root and turns a non-zero exit into an error.
fn cargo(args: &[&str]) -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));
    println!("\n▶ cargo {}", args.join(" "));
    let status = Command::new(&cargo)
        .args(args)
        .current_dir(workspace_root())
        .status()
        .map_err(|err| format!("could not run `{cargo}`: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`cargo {}` failed ({status})", args.join(" ")))
    }
}

/// The workspace root is the parent of this crate's directory.
fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().unwrap_or(manifest_dir).to_path_buf()
}
