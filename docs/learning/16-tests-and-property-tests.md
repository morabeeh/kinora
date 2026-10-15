# 16 — Unit, integration and property tests (≈ xUnit + FsCheck)

**In plain words:** Rust tests are ordinary functions marked `#[test]`. Unit tests live next to the code they test; integration tests live in a `tests/` folder and only see the public API. Property tests generate thousands of random inputs and check a rule holds for all of them.

| C# | Rust | In Kinora |
|---|---|---|
| a separate `*.Tests` project | `#[cfg(test)] mod tests { … }` at the bottom of the same file (can test private functions) | every module in `kinora_core` |
| `[Fact]` | `#[test] fn name() { … }` | — |
| `Assert.Equal(expected, actual)` | `assert_eq!(actual, expected)` | — |
| a test project that only uses the public API | `tests/*.rs` (integration tests) | [`kinora_decode/tests/fixtures.rs`](../../crates/kinora_decode/tests/fixtures.rs) |
| FsCheck `[Property]` | `proptest! { #[test] fn … (a in "regex") { … } }` | `natural_sort.rs`, `viewport.rs` |
| `dotnet test` | `cargo test --workspace` (or `cargo xtask check-all`) | — |

## Property tests found what examples missed
- Natural sort: "for any three random names, sorting keeps a < b < c consistent" and "two names compare equal only if they are identical". Writing these as rules is easier than inventing every tricky example.
- Viewport maths: "for any picture size, window size and zoom, the visible part stays inside the picture", and "fit is never pannable". The second one was added after a real bug (arrow keys panned by a fraction of a pixel at fit); the property test checks every size, not just the one that failed.

When a property fails, `proptest` **shrinks** the input to the smallest failing case and saves it, so the next run tries it first, like FsCheck's shrinking.

## Fixture tests
The decode tests open real files from `test_data/images` (generated, CC0): every format, all 8 EXIF orientations, and deliberately broken files. A broken file passes if it gives an error (or a partial picture) **quickly** and never panics. That's how the `avif-parse` panic was found (note 14).
