# 10 — Enums with data and `match` (≈ discriminated unions + switch expressions)

**In plain words:** a Rust `enum` is not just named numbers. Each case can carry its own data, like a sealed class hierarchy in C#, and `match` must handle every case or the code doesn't compile.

```rust
// crates/kinora_core/src/navigation.rs
pub enum NavOutcome {
    Moved { from: Option<usize>, to: usize, wrapped: bool },
    EndReached(Direction),
    Unchanged,
}
```
C# equivalent: `abstract record NavOutcome; record Moved(int? From, int To, bool Wrapped) : NavOutcome; record EndReached(Direction D) : NavOutcome; …`, and a `switch` expression over it.

| C# | Rust | In Kinora |
|---|---|---|
| `switch` expression with patterns | `match value { Pattern => …, }` | `Navigator::apply` matches `(command, current_index)` pairs |
| `default:` / discard `_` | `_ => …` | — |
| compiler warning for missing cases (sometimes) | **compile error** for missing cases | add a new `ErrorKind`, and every `match` on it must handle it |
| `if (x is Foo { Bar: > 3 } f)` | `if let Foo { bar, .. } = x && bar > 3 { … }` | `Viewer::on_decoded` |
| `int?` | `Option<usize>`: `Some(3)` or `None` | current index of an empty folder |

## State machines become hard to get wrong
The navigator is a small state machine: the inputs are `NavCommand` (Next, Previous, First, Last, GoTo), and every combination of command and position is one line in a `match`. Forgetting the "empty folder" case is a compile error, not a crash in the field.

## Errors are enums too
`DecodeError` in [`kinora_decode/src/error.rs`](../../crates/kinora_decode/src/error.rs) lists every way a picture can fail (`Empty`, `TooLarge { width, height }`, `Corrupt { format, detail }`, …). The app turns each case into a friendly card in `Failure::from_error` ([`viewer/jobs.rs`](../../crates/kinora_app/src/viewer/jobs.rs)), again with a `match` that must cover them all.
