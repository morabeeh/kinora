# 07 — Threads, channels, `Send`/`Sync` (≈ Thread, BlockingCollection, Dispatcher)

**In plain words:** the UI thread must never wait. Slow work (decoding, software video rendering) runs on other threads and hands *finished results* back to the UI thread. The compiler checks that only thread-safe data crosses threads.

| C# | Rust | In the spikes |
|---|---|---|
| `new Thread(() => …).Start()` | `std::thread::Builder::new().name("…").spawn(move \|\| …)` | `spawn_render_thread` (`spike/s4-video-sw`) |
| `BlockingCollection<T>` / `Channel<T>` | `std::sync::mpsc::channel()` → `(Sender, Receiver)` | mpv's "new frame" wake-ups |
| `Dispatcher.Invoke(() => …)` | `slint::invoke_from_event_loop(…)` or `weak.upgrade_in_event_loop(\|app\| …)` | handing a finished frame to the UI |
| `lock (x) { … }` | `Mutex<T>`: the data lives *inside* the lock | `Arc<Mutex<Stats>>` |
| `Interlocked` / `volatile bool` | `AtomicBool`, `AtomicU64` | stop flag, target size |
| shared object | `Arc<T>` (thread-safe reference count) | `Arc::clone(&stats)` |

## `Send` and `Sync`: thread safety the compiler checks
- `Send` = "this value may be **moved** to another thread". `Sync` = "a `&` reference may be **shared** between threads".
- Most types get them automatically. Raw pointers don't, so `SwRenderContext` says `unsafe impl Send` with a `// SAFETY:` comment: "only the render thread touches it after creation".
- Slint's UI objects are **not** `Send`, so you physically cannot touch a window from a worker thread. The compiler stops you, like WPF's "calling thread cannot access this object", but at build time instead of at runtime.
- That's why the render thread sends a `SharedPixelBuffer` (which is `Send`) and the UI thread turns it into a `slint::Image`.

## `Rc` vs `Arc`
`Rc<RefCell<T>>` = single-thread shared + mutable (the UI thread's state). `Arc<Mutex<T>>` = the multi-thread version. Using `Rc` across threads is a compile error, never a race.
