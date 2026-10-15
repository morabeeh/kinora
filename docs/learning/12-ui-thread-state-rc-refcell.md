# 12 — UI-thread state: `Rc<RefCell<…>>` and `thread_local!` (≈ a ViewModel + `Dispatcher` + `[ThreadStatic]`)

**In plain words:** the viewer's state (current folder, cache, zoom) belongs to the UI thread only. Worker threads never touch it; they post a closure that runs on the UI thread later, like `Dispatcher.BeginInvoke`.

The code is in [`viewer/mod.rs`](../../crates/kinora_app/src/viewer/mod.rs).

| C# | Rust | In Kinora |
|---|---|---|
| a ViewModel object shared by event handlers | `Rc<Viewer>`: single-thread reference counting (cheaper than `Arc`) | `viewer::install(Rc::clone(&viewer))` |
| mutating fields of `this` | `RefCell<State>`: `state.borrow_mut()` checks at **run time** that nobody else is reading or writing | `self.state.borrow_mut()` |
| `[ThreadStatic] static Viewer current;` | `thread_local! { static VIEWER: … }` | `with_viewer(\|v\| …)` |
| `Dispatcher.BeginInvoke(() => …)` | `slint::invoke_from_event_loop(move \|\| …)` | `post(\|viewer\| viewer.on_decoded(outcome))` |

## Why not just `&mut self`?
Slint calls our callbacks (key press, wheel, timer) with no access to our struct, so the struct is shared through `Rc` and mutated through `RefCell`. `RefCell` is a *run-time* borrow check: two `borrow_mut()` at the same time panics. That's why the code keeps borrows short — `{ let mut state = self.state.borrow_mut(); … }` — and releases them before calling another method that may borrow again. `with_viewer` clones the `Rc` and releases the thread-local *before* calling you, for the same reason.

## Why the controller isn't `Send`
`Rc` and `RefCell` aren't thread-safe, so the compiler refuses to move them to another thread (note 07). Worker threads only ever carry `Send` data (paths, decoded pictures in an `Arc`, pixel buffers) and ask the UI thread to do the rest.
