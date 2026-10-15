# 00 — Start here

These notes explain each Rust concept the first time Kinora uses it. Each note compares it with C# and points at the real code. Read them in order. Each one takes about 5 minutes.

| # | Note | C# idea it maps to |
|---|---|---|
| 01 | [Cargo workspace](01-cargo-workspace.md) | `.sln` + `.csproj` + `Directory.Build.props` |
| 02 | [`Result`, `?` and `anyhow`](02-result-and-anyhow.md) | exceptions, `try/catch`, rethrow |
| 03 | [Slint basics](03-slint-basics.md) | XAML, bindings, source generators |
| 04 | [Lints and clippy](04-lints-and-clippy.md) | Roslyn analyzers, `.editorconfig`, `TreatWarningsAsErrors` |
| 05 | [FFI and `unsafe`](05-ffi-and-unsafe.md) | P/Invoke, `SafeHandle`, `unsafe` blocks |
| 06 | [Traits and `dyn`](06-traits-and-dyn.md) | interfaces, DI, generics constraints |
| 07 | [Threads, channels, `Send`/`Sync`](07-threads-channels-send-sync.md) | `Thread`, `BlockingCollection`, `Dispatcher.Invoke` |
| 08 | [Build scripts and linking](08-build-scripts-and-linking.md) | `NativeLibrary`, import `.lib` files, NuGet native assets |
| 09 | [Ordering and sorting](09-ordering-and-sorting.md) | `IComparer<T>`, `ThenBy`, `PriorityQueue` |
| 10 | [Enums with data and `match`](10-enums-and-match.md) | discriminated unions, `switch` expressions, `int?` |
| 11 | [A worker pool: `Arc`, `Mutex`, `Condvar`](11-worker-pool-mutex-condvar.md) | `BlockingCollection`, `lock`, `Monitor.Wait/Pulse` |
| 12 | [UI-thread state: `Rc<RefCell>`](12-ui-thread-state-rc-refcell.md) | ViewModel, `Dispatcher.BeginInvoke`, `[ThreadStatic]` |
| 13 | [`serde` and the settings file](13-serde-settings.md) | `System.Text.Json` attributes |
| 14 | [Errors vs panics, `catch_unwind`](14-panics-and-catch-unwind.md) | expected exceptions vs bugs, `UnhandledException` |
| 15 | [`#[cfg]`: per-OS code and features](15-cfg-and-per-os-code.md) | `#if WINDOWS`, conditional `PackageReference` |
| 16 | [Unit, integration and property tests](16-tests-and-property-tests.md) | xUnit, FsCheck |

## Where to start reading the code
1. [`crates/kinora_app/src/main.rs`](../../crates/kinora_app/src/main.rs) is the entry point, like `Program.cs`.
2. [`crates/kinora_app/ui/app_window.slint`](../../crates/kinora_app/ui/app_window.slint) is the window, like `MainWindow.xaml`.
3. [`Cargo.toml`](../../Cargo.toml) (root) is the workspace, its shared dependencies and the lint rules.
4. [`crates/kinora_app/src/viewer/mod.rs`](../../crates/kinora_app/src/viewer/mod.rs) is the viewer controller (navigation, decoding, zoom) — the "ViewModel".
5. [`crates/kinora_core/src/`](../../crates/kinora_core/src/) holds the pure logic with its tests; [`crates/kinora_decode/src/lib.rs`](../../crates/kinora_decode/src/lib.rs) is the decoding pipeline.

## Daily commands
```
cargo check                 # fast "does it compile?"  (≈ dotnet build without output)
cargo run -p kinora_app     # build + run               (≈ dotnet run --project)
cargo test --workspace      # all tests                 (≈ dotnet test)
cargo xtask check-all       # everything CI checks
```
Also see `RUST_FOR_DOTNET_DEVS.md` in the repo root for the big-picture comparison table.
