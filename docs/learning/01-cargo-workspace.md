# 01 — Cargo workspace (≈ a .sln with Directory.Build.props)

**In plain words:** a workspace is one folder holding several projects ("crates") that are built together and share one lock file and one `target/` output folder.

| C# / .NET | Rust / Cargo | In Kinora |
|---|---|---|
| `Kinora.sln` | root `Cargo.toml` with `[workspace]` | [`Cargo.toml`](../../Cargo.toml) |
| `Foo.csproj` | `crates/foo/Cargo.toml` (a *crate*) | [`crates/kinora_core/Cargo.toml`](../../crates/kinora_core/Cargo.toml) |
| Class library / Exe | `src/lib.rs` = library, `src/main.rs` = binary | `kinora_core` (lib), `kinora_app` (bin) |
| `Directory.Build.props` | `[workspace.package]` + `x.workspace = true` | edition, license, version shared |
| Central Package Management (`Directory.Packages.props`) | `[workspace.dependencies]` + `dep = { workspace = true }` | all versions pinned in the root |
| `packages.lock.json` | `Cargo.lock` (committed for apps) | root `Cargo.lock` |
| `global.json` (SDK pin) | `rust-toolchain.toml` | pins Rust 1.99.0 |
| `bin/` + `obj/` | one shared `target/` folder | ignored by git |
| NuGet feature flags (rare) | **cargo features**: optional parts of a crate you switch on | `slint` features: `renderer-femtovg`, `renderer-software` |

## Things that differ from .NET
- **Project references** use paths: `kinora_core = { path = "crates/kinora_core" }`. We declare them once in the root and then write `kinora_core = { workspace = true }` in a member.
- **Edition** (`edition = "2024"`) is a language-version switch, like `<LangVersion>`. Crates on different editions still link together fine.
- `members = ["crates/*", "xtask"]` uses a glob, so a new folder under `crates/` joins automatically.
- `cargo run -p kinora_app` picks the package (`-p`), like `dotnet run --project`.

## Profiles (≈ Debug/Release configurations)
In [`Cargo.toml`](../../Cargo.toml), `[profile.release]` turns on link-time optimisation. `[profile.dev.package."*"] opt-level = 2` builds *dependencies* optimised even in debug, so image decoding isn't 50× slower while you develop, but *our* code still compiles fast and debugs nicely.
