# 06 — Traits and `dyn` (≈ interfaces and dependency injection)

**In plain words:** a *trait* is Rust's interface. The three video backends (A: OpenGL texture, B: child window, C: software buffer) will implement one `VideoOutput` trait, so the rest of the app doesn't care which one is running. That's like `IVideoOutput` with three classes behind it.

```rust
// Phase 3 sketch (kinora_video), not code yet:
pub trait VideoOutput {
    fn attach(&mut self, window: &WindowInfo) -> Result<(), VideoError>;
    fn resize(&mut self, width: u32, height: u32);
    fn supports_overlay(&self) -> bool;   // A and C: true, B: false (controls go below)
}
pub struct GlTextureOutput { /* … */ }     // backend A
impl VideoOutput for GlTextureOutput { /* … */ }
```

| C# | Rust |
|---|---|
| `interface IVideoOutput` | `trait VideoOutput` |
| `class GlTextureOutput : IVideoOutput` | `impl VideoOutput for GlTextureOutput` |
| `IVideoOutput output = new GlTextureOutput();` | `let output: Box<dyn VideoOutput> = Box::new(GlTextureOutput::new());` |
| generic `void Use<T>(T x) where T : IVideoOutput` | `fn use_it<T: VideoOutput>(x: T)` (resolved at compile time, no virtual call) |
| extension methods need a `using` | trait methods need the trait in scope (`use slint::ComponentHandle as _;`) |

## `dyn` vs generics
- `Box<dyn VideoOutput>` = runtime choice through a virtual table, like a C# interface reference. We need this because the backend is picked at runtime from the hardware profile.
- `T: VideoOutput` = the compiler generates a copy per type. It's faster, but the choice is fixed at compile time.

## Where you've already used a trait
`use slint::ComponentHandle as _;` in `crates/kinora_app/src/main.rs` imports the trait that provides `window.run()`. In the spikes, `glow::HasContext` gives all the GL functions (`gl.create_texture()`), and `raw_window_handle::HasWindowHandle` gives the native HWND (`spike/s3-video-wid`).
