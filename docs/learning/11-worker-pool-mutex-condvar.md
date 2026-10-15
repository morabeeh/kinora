# 11 — A worker pool: `Arc`, `Mutex`, `Condvar` (≈ `BlockingCollection` + `Monitor.Wait/Pulse`)

**In plain words:** a few background threads wait for work. When a job arrives, one wakes up, takes the most urgent job and runs it. When the user moves on, jobs nobody needs are thrown away before they start.

The code is in [`kinora_cache/src/pool.rs`](../../crates/kinora_cache/src/pool.rs) (decode jobs) and [`viewer/render.rs`](../../crates/kinora_app/src/viewer/render.rs) (the one-slot "latest frame wins" mailbox).

| C# | Rust | In Kinora |
|---|---|---|
| `lock (gate) { … }` | `let state = mutex.lock()` — the data lives **inside** the `Mutex`; the guard unlocks when it goes out of scope | `Shared::lock` |
| `Monitor.Wait(gate)` | `state = condvar.wait(state)` (releases the lock while waiting) | `worker()` loop |
| `Monitor.Pulse` / `PulseAll` | `condvar.notify_one()` / `notify_all()` | `submit`, `Drop` |
| shared object across threads | `Arc<Shared>` — a thread-safe reference count | each worker owns an `Arc::clone` |
| `Thread.Join()` on dispose | `impl Drop for WorkPool` joins the threads | dropping the pool at shutdown |
| `catch (Exception)` around a task | `catch_unwind` around a job (note 14) | a panicking decoder doesn't kill the worker |

## Why the data is *inside* the lock
In C#, nothing stops you from touching `queue` without `lock (gate)`. In Rust, the queue is only reachable through the `MutexGuard`, so forgetting the lock is a compile error. A "poisoned" lock (another thread panicked while holding it) is handled explicitly with `unwrap_or_else(PoisonError::into_inner)`, because every update of our queue is a single push or pop that can't leave it half-changed.

## Cancellation by re-ranking
`reprioritize(|key| …)` goes over the queued jobs: return `Some(priority)` to keep a job (perhaps more urgent now), `None` to drop it. The viewer calls it on every navigation with "current picture first, then the neighbours", so a fast flick never waits for pictures the user already skipped.
