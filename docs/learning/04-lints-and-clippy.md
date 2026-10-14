# 04 — Lints and clippy (≈ Roslyn analyzers + TreatWarningsAsErrors)

**In plain words:** `rustc` warns about suspicious code, and **clippy** is an extra linter with hundreds of rules. We configure them once for the whole workspace and treat warnings as errors in CI.

| .NET | Rust | In Kinora |
|---|---|---|
| Roslyn analyzers / StyleCop | `clippy` (`cargo clippy`) | `[workspace.lints.clippy]` in [`Cargo.toml`](../../Cargo.toml) |
| `.editorconfig` severity (`dotnet_diagnostic.CA1062.severity = error`) | `unwrap_used = "deny"` | same section |
| `<TreatWarningsAsErrors>` | `cargo clippy ... -- -D warnings` | `cargo xtask check-all`, CI |
| `#pragma warning disable` / `[SuppressMessage]` | `#[allow(clippy::xyz)]` | only with a reason comment (see `xtask/src/main.rs`) |
| `dotnet format` | `cargo fmt` (`rustfmt.toml`) | `cargo fmt --all` |
| `<Nullable>enable</Nullable>` making null-safety strict | built into the language: no null, `Option<T>` | — |

Every crate opts in with:
```toml
[lints]
workspace = true
```

## The rules that will surprise a C# developer
- **`indexing_slicing`**: `v[3]` is banned because it panics if out of range. Use `v.get(3)`, which returns `Option` (`None` if missing). It's like always using `TryGetValue`.
- **`as_conversions`**: `x as u32` silently truncates or wraps, like an `unchecked` cast. Use `u32::try_from(x)?`. Pixel maths with width×height overflows for real on huge images.
- **`unwrap_used` / `expect_used` / `panic`**: no "crash if unexpected". Handle the error (note 02).
- **`print_stdout` / `print_stderr`**: no `println!`. Use `tracing::info!(...)`, a structured logger like `ILogger`.

`clippy.toml` relaxes unwrap/expect/indexing/panic **inside tests only**, because a panic there just means the test failed. `xtask` (a dev-only command-line tool) is the one crate allowed to print. That exception is commented at the top of [`xtask/src/main.rs`](../../xtask/src/main.rs).
