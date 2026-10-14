# 00 — Start here

These notes explain each Rust concept the first time Kinora uses it. Each note compares it with C# and points at the real code. Read them in order. Each one takes about 5 minutes.

| # | Note | C# idea it maps to |
|---|---|---|
| 01 | [Cargo workspace](01-cargo-workspace.md) | `.sln` + `.csproj` + `Directory.Build.props` |
| 02 | [`Result`, `?` and `anyhow`](02-result-and-anyhow.md) | exceptions, `try/catch`, rethrow |
| 03 | [Slint basics](03-slint-basics.md) | XAML, bindings, source generators |
| 04 | [Lints and clippy](04-lints-and-clippy.md) | Roslyn analyzers, `.editorconfig`, `TreatWarningsAsErrors` |

## Where to start reading the code
1. [`crates/kinora_app/src/main.rs`](../../crates/kinora_app/src/main.rs) is the entry point, like `Program.cs`.
2. [`crates/kinora_app/ui/app_window.slint`](../../crates/kinora_app/ui/app_window.slint) is the window, like `MainWindow.xaml`.
3. [`Cargo.toml`](../../Cargo.toml) (root) is the workspace, its shared dependencies and the lint rules.

## Daily commands
```
cargo check                 # fast "does it compile?"  (≈ dotnet build without output)
cargo run -p kinora_app     # build + run               (≈ dotnet run --project)
cargo test --workspace      # all tests                 (≈ dotnet test)
cargo xtask check-all       # everything CI checks
```
Also see `RUST_FOR_DOTNET_DEVS.md` in the repo root for the big-picture comparison table.
