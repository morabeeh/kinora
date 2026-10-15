# 13 — `serde`: settings files (≈ `System.Text.Json` with attributes)

**In plain words:** `#[derive(Serialize, Deserialize)]` generates the code that turns a struct into TOML text and back, like `JsonSerializer` with attributes, but at compile time (no reflection).

The code is in [`kinora_core/src/settings.rs`](../../crates/kinora_core/src/settings.rs); reading and saving the file is in [`kinora_app/src/settings_store.rs`](../../crates/kinora_app/src/settings_store.rs).

| C# (`System.Text.Json`) | Rust (`serde`) | Effect in `settings.toml` |
|---|---|---|
| `[JsonPropertyName("wrap-around")]` on every property | `#[serde(rename_all = "kebab-case")]` once on the struct | `wrap-around = true` |
| default values when a property is missing | `#[serde(default)]` + `impl Default` | an old settings file still loads |
| `JsonIgnoreCondition.WhenWritingNull` | `#[serde(skip_serializing_if = "Option::is_none")]` | absent keys stay absent |
| enum as string (`JsonStringEnumConverter`) | plain enum + `rename_all` | `wheel = "zoom"` |
| unknown properties ignored | ignored by default | a newer Kinora's settings don't break an older one |

## Pure model, I/O elsewhere
`Settings::from_toml` / `to_toml` only convert text; they never touch the disk, so the tests in the same file can check defaults and round trips without temporary files. `SettingsStore` does the I/O: it saves with an **atomic write** (`kinora_platform::fs::atomic_write`: temp file → flush → rename) and never overwrites a file it couldn't read, so a typo in a hand-edited file isn't silently erased.
