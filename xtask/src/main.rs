//! Developer task runner, invoked as `cargo xtask <command>` (alias in `.cargo/config.toml`).
//!
//! Think of it as the project's build script written in Rust instead of PowerShell/Bash,
//! so the same commands work on Linux, Windows and macOS.

// xtask is a command-line developer tool that is never shipped to users. Printing to the
// terminal *is* its user interface, so the workspace ban on print macros does not apply here.
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod gen_folder;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const HELP: &str = "\
Usage: cargo xtask <command>

Commands:
  check-all              fmt --check, clippy (-D warnings), tests, cargo deny — stops at the first failure
  run [--release] [-- <kinora args>]
                         build and run Kinora with the media DLLs on PATH (Windows)
  gen-folder <dir> [--count N] [--megapixels M]
                         write N mixed-format test photos (default 500, ≈12 MP max) for flick tests
  licenses               regenerate THIRD_PARTY_NOTICES.md with cargo-about
  help                   show this message

On Windows, commands that build add the default tool locations from docs/setup-windows.md
(vcpkg's libheif, NASM) to the environment when they are not set already.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("check-all") => check_all(),
        Some("run") => run(args.get(1..).unwrap_or_default()),
        Some("gen-folder") => gen_folder::run(args.get(1..).unwrap_or_default()),
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

/// `cargo xtask run [--release] [-- args]` → `cargo run -p kinora_app [--release] -- args`.
fn run(args: &[String]) -> Result<(), String> {
    let mut cargo_args = vec!["run", "-p", "kinora_app"];
    let mut rest = args.iter().map(String::as_str);
    for arg in rest.by_ref() {
        match arg {
            "--release" => cargo_args.push("--release"),
            "--" => break,
            other => {
                return Err(format!(
                    "unknown `run` option `{other}` (app arguments go after `--`)"
                ));
            }
        }
    }
    cargo_args.push("--");
    cargo_args.extend(rest);
    cargo(&cargo_args)
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
        .envs(dev_environment())
        .current_dir(workspace_root())
        .status()
        .map_err(|err| format!("could not run `{cargo}`: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`cargo {}` failed ({status})", args.join(" ")))
    }
}

/// Extra environment for child processes on Windows: the vcpkg root (libheif), and `PATH`
/// entries for NASM (needed to build the AVIF decoder) and the libheif DLLs (needed to run
/// Kinora and its tests). Only defaults that exist on disk and aren't configured already are
/// added, so a developer's own settings always win. Elsewhere, nothing is changed.
fn dev_environment() -> Vec<(String, OsString)> {
    let mut env = Vec::new();
    if !cfg!(windows) {
        return env;
    }
    let tools = std::env::var_os("USERPROFILE")
        .map(|home| PathBuf::from(home).join("dev").join("kinora-tools"));

    let vcpkg_root = std::env::var_os("VCPKG_ROOT")
        .map(PathBuf::from)
        .or_else(|| {
            let default = tools.as_ref()?.join("vcpkg");
            default.is_dir().then(|| {
                env.push((String::from("VCPKG_ROOT"), default.clone().into_os_string()));
                default
            })
        });

    let mut extra_path: Vec<PathBuf> = Vec::new();
    if let Some(root) = &vcpkg_root {
        extra_path.push(root.join("installed").join("x64-windows").join("bin"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        extra_path.push(PathBuf::from(local).join("bin").join("NASM"));
    }
    let current = std::env::var_os("PATH").unwrap_or_default();
    let existing: Vec<PathBuf> = std::env::split_paths(&current).collect();
    let mut paths: Vec<PathBuf> = extra_path
        .into_iter()
        .filter(|dir| dir.is_dir() && !existing.contains(dir))
        .collect();
    if !paths.is_empty() {
        paths.extend(existing);
        if let Ok(joined) = std::env::join_paths(paths) {
            env.push((String::from("PATH"), joined));
        }
    }
    env
}

/// The workspace root is the parent of this crate's directory.
fn workspace_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().unwrap_or(manifest_dir).to_path_buf()
}
