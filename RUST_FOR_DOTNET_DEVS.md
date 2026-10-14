# Rust for a .NET Developer — What You Need to Know for Kinora

You don't need to master Rust before starting. Claude Code will write most of the code; your job is to read it, run it, test it, and steer. This guide gives you enough to do that confidently, and tells you what to learn first.

---

## 1. The 10-minute mental model

| .NET / C# | Rust | What's different |
|---|---|---|
| `dotnet` CLI | `cargo` | Same idea: build, run, test, add packages. `cargo build`, `cargo run`, `cargo test`. |
| NuGet package | **crate** (from crates.io) | Added in `Cargo.toml` instead of `.csproj`. |
| Solution (`.sln`) | **workspace** | One root `Cargo.toml` listing member crates. Kinora uses one. |
| Project / assembly | crate (library or binary) | `kinora_core`, `kinora_video`… are like separate class-library projects. |
| `namespace` | `mod` (module) | Modules follow the file tree. `pub` = public, default is private. |
| Garbage collector | **ownership** | No GC. Every value has one owner; it is freed the moment the owner goes out of scope. |
| `IDisposable` / `using` | `Drop` | Cleanup runs automatically and deterministically at end of scope. No `using` needed. |
| References (`obj`) | **borrows** `&T` (read) and `&mut T` (write) | You can have many readers *or* one writer at a time — never both. The compiler enforces it. |
| `null`, `T?` | `Option<T>` (`Some(x)` / `None`) | There is no null. You must handle the "missing" case explicitly. |
| Exceptions | `Result<T, E>` (`Ok` / `Err`) | Errors are return values. The `?` operator means "if this failed, return the error to my caller" — like an automatic rethrow. |
| `interface` | `trait` | Very similar. Traits can also be implemented for types you didn't write. |
| C# `enum` | `enum` (a "sum type") | Rust enums can carry data per variant: `enum Media { Image(ImageInfo), Video(VideoInfo) }`. Think of a sealed class hierarchy in one line. |
| `switch` expression / pattern matching | `match` | Must cover every case — the compiler checks. |
| Generics | Generics | Similar, but compiled per type (fast, no boxing). |
| LINQ | Iterators | `items.iter().filter(...).map(...).collect()` — lazy, like LINQ, and very fast. |
| Lambdas | Closures | `|x| x + 1`. |
| `List<T>` / `Dictionary<K,V>` | `Vec<T>` / `HashMap<K,V>` | |
| `string` | `String` (owned) and `&str` (borrowed view) | Two kinds: you own a `String`; you borrow a `&str`. Most confusion at first comes from this. |
| `lock` / `Monitor` | `Mutex<T>` | The lock *wraps the data* — you literally can't touch the data without locking. |
| Shared object across threads | `Arc<T>` | Thread-safe reference counting (like a shared pointer). Often `Arc<Mutex<T>>`. |
| `Task` / `async` | threads (and `async`, which Kinora avoids) | Kinora uses plain worker threads + channels. Simpler to learn and debug. |
| Roslyn analyzers | `clippy` | Very good linter. We treat its warnings as errors. |
| `dotnet format` | `rustfmt` / `cargo fmt` | |
| xUnit / NUnit | built-in `#[test]` + `cargo test` | Tests often live in the same file, in a `mod tests` block. |
| P/Invoke / `unsafe` | FFI + `unsafe` | Calling C libraries (libmpv, libheif). Kinora confines this to two crates. |
| XAML (WPF/WinUI/MAUI) | **Slint `.slint` files** | Declarative UI with properties, bindings and callbacks. Closest thing to XAML you'll find in Rust. |
| `INotifyPropertyChanged` | Slint properties | Bindings update automatically when a property changes. |
| `ICommand` / event handlers | Slint `callback`s | Declared in `.slint`, implemented in Rust with `on_xxx(...)`. |
| `Dispatcher.Invoke` | `slint::invoke_from_event_loop` | How a background thread hands a result to the UI thread. |

---

## 2. The three things that will feel strange

**1. The borrow checker will reject code that "obviously works."**
It's preventing a data race or use-after-free that C# would catch at runtime (or never). When it happens:
- Clone small things freely (`.clone()` on a `String` or small struct is fine).
- Pass IDs or indexes instead of long-lived references.
- Ask Claude Code: *"Explain this borrow error in C# terms and show me the minimal fix."*

**2. No exceptions.**
Functions that can fail return `Result`. You'll see `?` everywhere. Kinora forbids `.unwrap()` in non-test code (it's like ignoring an exception and crashing), so errors always get handled and shown nicely.

**3. Compile times and strictness.**
The compiler is slow-ish and very strict, but once it compiles, whole classes of bugs (null refs, data races, use-after-free) are gone. Use `cargo check` for fast feedback; full `cargo build` only when you want to run.

---

## 3. Commands you'll use daily

```bash
cargo check                 # fast "does it compile?" (no binary)
cargo run -p kinora_app -- ~/Pictures/test.jpg   # run the app with a file
cargo run --release -p kinora_app -- file.mkv    # run optimized (use for performance testing)
cargo test --workspace      # all tests
cargo clippy --workspace --all-targets -- -D warnings   # linter
cargo fmt                   # format everything
cargo deny check            # licenses + security advisories
cargo xtask check-all       # Kinora's one-shot "is everything green?" (set up in Phase 0)
rustup update               # update the Rust toolchain
```

**Debug vs release matters a lot for images.** Debug builds can be 10–50× slower at decoding. Kinora's dev profile optimizes dependencies to soften this, but always judge speed with `--release`.

---

## 4. Installing Rust (Phase 0 will walk you through it — this is the preview)

**Linux Mint**
```bash
sudo apt update
sudo apt install build-essential pkg-config git curl cmake
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# restart the terminal, then:
rustc --version && cargo --version
```
Claude Code will add the exact media/UI development packages (libmpv, libheif, fontconfig, xkbcommon, etc.) after checking the right package names for your Mint version.

**Windows**
1. Install *Visual Studio Build Tools* with the "Desktop development with C++" workload (Rust uses Microsoft's linker on Windows — you may already have this from .NET work).
2. Download and run `rustup-init.exe` from rustup.rs.

**Editor:** VS Code with **rust-analyzer** (IntelliSense equivalent), **Slint** (live UI preview while you edit `.slint` files), **CodeLLDB** (debugger — breakpoints work like in Visual Studio), **Even Better TOML**. JetBrains **RustRover** is a good alternative if you prefer Rider.

---

## 5. How to read Kinora's code

- Start in `crates/kinora_app/src/main.rs` — it wires everything together, like `Program.cs`.
- `crates/kinora_app/ui/*.slint` — the screens. Read these like XAML.
- `crates/kinora_core` — pure logic with tests. The easiest place to learn Rust because there's no UI or C code in it.
- `crates/kinora_video` and `kinora_platform` — the only places with `unsafe`. Read them last.
- `docs/learning/` — notes Claude Code writes for you as each new concept appears, each with a C# comparison and a pointer to real Kinora code.

---

## 6. Learning path (about 2–3 weeks part-time, alongside the build)

1. **Microsoft's "Rust for C#/.NET Developers"** — a short guide written by .NET developers that maps C# concepts to Rust. Read it first: https://microsoft.github.io/rust-for-dotnet-devs/latest/
2. **The Rust Book**, chapters 1–10 and 13, 15, 16 (ownership, structs, enums, errors, generics, traits, iterators, smart pointers, concurrency): https://doc.rust-lang.org/book/
3. **Rustlings** — small exercises you fix until they compile: https://github.com/rust-lang/rustlings
4. **Slint docs** — the language reference and the Rust API: https://slint.dev/docs
5. **Rust by Example** for quick lookups: https://doc.rust-lang.org/rust-by-example/

Skip for now: async/await, macros (writing them), lifetimes beyond the basics, unsafe. You'll read them in Kinora's code, but you don't need to write them.

---

## 7. Working well with Claude Code on this project

- Keep sessions scoped to one phase. Start each session with: *"Read CLAUDE.md and PROJECT_BRIEF.md. We're on Phase N. Summarize where we left off."*
- Ask for explanations freely: *"Walk me through `navigation.rs` as if I were reviewing a C# PR."*
- Before approving a plan, check it against the acceptance criteria in the brief.
- Actually run the manual test checklist at the end of each phase — especially on your real Mint machine and GPU. Headless tests can't tell you if video stutters.
- If something feels wrong in the UI, describe it in terms of what you expected (*"Windows Photos shows the next image instantly here; we flash black"*). That gives Claude a concrete target.
- Commit often. If a phase goes sideways, `git` lets you go back.

---

## 8. Glossary you'll see in the code

- **crate** — a package/library. **workspace** — a group of crates built together.
- **trait** — interface. **impl** — the implementation block for a type or trait.
- **`&` / `&mut`** — borrow for reading / for writing.
- **`Box<T>`** — a heap allocation with one owner. **`Arc<T>`** — shared ownership across threads. **`Rc<T>`** — shared ownership on one thread.
- **`Send` / `Sync`** — compiler-checked markers meaning "safe to move to / share between threads."
- **`#[derive(Debug, Clone)]`** — auto-generate common trait implementations (like source generators).
- **`macro!`** — anything with `!` is a macro (`println!`, `vec!`, `slint::include_modules!`).
- **FFI** — calling C libraries (mpv, libheif). **`unsafe`** — "compiler, trust me here"; always explained with a `// SAFETY:` comment in Kinora.
- **LTO** — link-time optimization for smaller, faster release builds.
