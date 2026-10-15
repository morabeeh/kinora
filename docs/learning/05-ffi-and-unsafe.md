# 05 — FFI and `unsafe` (≈ P/Invoke and `unsafe` blocks in C#)

**In plain words:** FFI ("foreign function interface") means calling a C library, here libmpv. The Rust compiler can't check C code, so the calls go inside `unsafe { … }`, and we write down *why* each one is correct.

| C# | Rust | In Kinora (branch `spike/phase1`) |
|---|---|---|
| `[DllImport("mpv")] static extern int mpv_command(...)` | `extern "C" { fn mpv_command(...) -> c_int; }` (here generated for us by `libmpv2-sys`) | `spike/s4-video-sw/src/main.rs` |
| `IntPtr` | raw pointers `*mut T` / `*const T` | `SwRenderContext(*mut sys::mpv_render_context)` |
| `unsafe { fixed (byte* p = buf) { … } }` | `unsafe { … }` plus `buf.as_mut_ptr()` | `SwRenderContext::render` |
| `SafeHandle` (wrapping a handle, freeing it in `ReleaseHandle`) | a struct owning the pointer + `impl Drop` | `impl Drop for SwRenderContext` → `mpv_render_context_free` |
| `[UnmanagedFunctionPointer]` delegate passed to C | `unsafe extern "C" fn on_update(data: *mut c_void)` | `set_update_callback` |
| `Marshal.StringToHGlobalAnsi` | C string literal `c"rgb0"` (already NUL-terminated) | `render()` |

## The rules we follow
1. **`unsafe` is allowed only in `kinora_video` and `kinora_platform`.** Everywhere else the compiler rejects it (`unsafe_code = "deny"`).
2. **Every `unsafe` block gets a `// SAFETY:` comment** that states the invariant, e.g. "the pointer is the context we created and haven't freed".
3. **Wrap it once, safely.** The rest of the app calls a safe method (`ctx.render(&mut buffer)`) and never sees a raw pointer, the same way you'd hide P/Invoke behind a managed class.

## Two traps Phase 1 hit
- **Lifetimes across FFI.** Slint lends us a GL loader function that is valid *only during one callback*. mpv wants it "forever". The compiler refused until we spelled out an explicit, documented lifetime extension (`transmute` to `'static`), which is safe only because mpv uses the loader inside that same callback. In C# this bug would compile and crash at runtime; here it couldn't compile without us writing the justification. See `GlVideo::new` in `spike/s2-video-gl/src/main.rs`.
- **Ownership of C objects.** mpv's render context *borrows* the mpv handle. The spike "leaked" the handle (`Box::leak`) to keep things simple. The real `kinora_video` will own both in one struct and drop them in the right order.
