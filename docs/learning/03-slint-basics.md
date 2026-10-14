# 03 — Slint basics (≈ XAML compiled by a source generator)

**In plain words:** you describe the UI in `.slint` files, much like XAML. At build time they're turned into Rust code that our `main.rs` calls.

| WPF / WinUI | Slint | In Kinora |
|---|---|---|
| `MainWindow.xaml` | `export component AppWindow inherits Window { … }` | [`ui/app_window.slint`](../../crates/kinora_app/ui/app_window.slint) |
| `ResourceDictionary` / theme resources | an `export global Theme { out property … }` | [`ui/theme.slint`](../../crates/kinora_app/ui/theme.slint) |
| `{Binding Foo}` / `INotifyPropertyChanged` | `property <T> foo;`, where bindings update automatically | Phase 2 onwards |
| `ICommand` / event handler | `callback clicked();` declared in `.slint`, implemented in Rust with `on_clicked(...)` | Phase 2 onwards |
| `x:Uid` / `.resx` localisation | `@tr("Kinora")` marks a string for translation | window title |
| `Dispatcher.Invoke` | `slint::invoke_from_event_loop(...)` | Phase 2 (decode results) |
| XAML compiler → generated `.g.cs` | **`build.rs`** runs `slint_build::compile(...)` before compiling the crate | [`build.rs`](../../crates/kinora_app/build.rs) |

## `build.rs` — a build script
If a crate has a `build.rs`, Cargo compiles and runs it **before** compiling the crate itself. It's similar to an MSBuild target or a C# source generator. Ours compiles the `.slint` files into Rust code inside `target/`.

## `slint::include_modules!()` — a macro
Anything ending in `!` is a **macro**: code that writes code at compile time. `include_modules!()` pastes the generated code in at that spot. We put it inside `mod ui { … }`, so the generated types are reached as `ui::AppWindow`, like a namespace.

## `ComponentHandle` — a trait you import to get methods
`window.run()` only compiles because of `use slint::ComponentHandle as _;`. In Rust, methods that come from a *trait* (≈ interface) are only callable when that trait is in scope. That's like C# extension methods needing their `using`. `as _` means "bring the methods in, but I don't need the name".

## Renderers
Both are compiled in. Pick one with an environment variable (Phase 2 makes this automatic):
- default: FemtoVG on OpenGL (GPU)
- `SLINT_BACKEND=winit-software`: the CPU renderer, for PCs without usable graphics drivers.

The VS Code **Slint** extension shows a live preview: open a `.slint` file and run "Slint: Show Preview".
