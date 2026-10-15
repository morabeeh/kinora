# 14 — Errors vs panics, and `catch_unwind` (≈ expected exceptions vs bugs)

**In plain words:** Rust has two kinds of failure. A `Result::Err` is an *expected* problem ("this file is damaged") that the caller must handle. A **panic** is a *bug* ("index out of range"), and by default it unwinds the thread, like an unhandled exception. Kinora's rule: bad input is always an `Err`, never a panic. But because decoders are written by others, we also put a safety net around them.

| C# | Rust | In Kinora |
|---|---|---|
| `throw new InvalidDataException(...)` for bad input | `return Err(DecodeError::Corrupt { … })` | every decoder in `kinora_decode` |
| an unhandled `NullReferenceException` | a panic (`unwrap` on `None`, out-of-range index, `panic!`) | our lints forbid these in our own code (note 04) |
| `try { … } catch (Exception) { … }` around a plugin call | `std::panic::catch_unwind(AssertUnwindSafe(\|\| …))` | `kinora_decode::decode` and each `WorkPool` job |
| `AppDomain.UnhandledException` logging | `std::panic::set_hook` | `install_panic_hook` in [`logging.rs`](../../crates/kinora_app/src/logging.rs) |

## Where the nets are
1. [`kinora_decode/src/lib.rs`](../../crates/kinora_decode/src/lib.rs): `decode()` runs the decoder inside `catch_unwind`; a panic becomes `DecodeError::Panicked(format)`, and the user sees an error card.
2. [`kinora_cache/src/pool.rs`](../../crates/kinora_cache/src/pool.rs): each job runs inside `catch_unwind`, so the worker thread survives and the app is told which job failed.
3. The panic hook writes every panic to the log file, even when it was caught.

## A real example
The fixture tests found that `avif-parse` (a dependency) **panics** on a truncated AVIF file. The net turned it into an error card, but we still fixed the cause: `isobmff::boxes_complete` rejects incomplete files before the decoder sees them. The net is for surprises, not for known problems.

`catch_unwind` only works because the release profile keeps `panic = "unwind"` (root `Cargo.toml`). With `panic = "abort"`, any panic would end the whole app.
