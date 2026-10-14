# 02 — `Result`, `?` and `anyhow` (≈ exceptions, but as return values)

**In plain words:** Rust has no exceptions. A function that can fail says so in its return type, `Result<T, E>`, and the caller *must* deal with it. The compiler won't let you silently ignore a failure.

```rust
// C#:   Window CreateWindow()            // might throw — signature doesn't tell you
// Rust: fn new() -> Result<AppWindow, PlatformError>   // failure is part of the signature
```

| C# | Rust |
|---|---|
| `throw new X()` | `return Err(x)` |
| `try { a(); } catch (Exception e) { … }` | `match a() { Ok(v) => …, Err(e) => … }` |
| letting the exception bubble up | **`?`**: `let w = AppWindow::new()?;` returns the error to *our* caller |
| `catch (Exception e) { throw new X("context", e); }` | `.context("could not create the main window")?` (anyhow) |
| `Exception` base class for "anything" | `anyhow::Error`, which can hold any error type plus context |
| custom exception classes | an `enum` of error cases with `thiserror` (library crates, from Phase 1 on) |
| `null` | `Option<T>`: `Some(x)` or `None` |

## In Kinora
[`crates/kinora_app/src/main.rs`](../../crates/kinora_app/src/main.rs):
```rust
fn main() -> anyhow::Result<()> {
    let window = ui::AppWindow::new().context("could not create the main window")?;
    window.run().context("the UI event loop failed")?;
    Ok(())   // () is "void"; Ok(()) means "succeeded, nothing to return"
}
```
If `main` returns an `Err`, the program exits with a non-zero code and prints the error chain, like an unhandled exception but without a crash.

## Our rules
- **`anyhow` only in the app binary.** Libraries return precise error enums (`thiserror`), so the app can map each case to a friendly message.
- **`.unwrap()` / `.expect()` are banned** outside tests. They mean "crash if this failed", which is like `catch {}` followed by `Environment.FailFast`.
- `unwrap_or_else(|_| default)` (see `init_logging`) is fine: it *handles* the error by choosing a fallback.
