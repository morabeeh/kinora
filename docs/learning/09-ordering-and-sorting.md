# 09 — Ordering and sorting (≈ `IComparer<T>`, `Comparison<T>`)

**In plain words:** to sort file names like Explorer does ("img2" before "img10"), we write one function that compares two names and returns *less*, *equal* or *greater*. Rust's sort calls it, just like `List<T>.Sort(Comparison<T>)`.

| C# | Rust | In Kinora |
|---|---|---|
| `int Compare(a, b)` → `-1 / 0 / 1` | `fn cmp(a, b) -> std::cmp::Ordering` → `Less / Equal / Greater` | `natural_cmp` in [`natural_sort.rs`](../../crates/kinora_core/src/natural_sort.rs) |
| `list.Sort((a, b) => …)` | `items.sort_by(\|a, b\| …)` (stable, like `OrderBy`) | `FolderModel::replace_items` in [`folder.rs`](../../crates/kinora_core/src/folder.rs) |
| `.ThenBy(…)` | `ordering.then_with(\|\| …)` | `compare_items`: sort key, then name, then path |
| `IComparable<T>` | `impl Ord for T` | `Queued` jobs in [`pool.rs`](../../crates/kinora_cache/src/pool.rs) |
| `Comparer<T>.Create(...).Reverse` | `ordering.reverse()` | descending sort |

## Why "total order" matters
`sort_by` assumes the comparison is consistent: if a < b and b < c then a < c, and two *different* names never compare equal. If that's broken, the order can change between runs (and some languages' sorts even throw). `natural_cmp` therefore ends with tie-breakers (`File1` vs `file1`, `a1` vs `a01`). The property tests in the same file check these rules on thousands of random names (see note 16).

## A max-heap that pops the *most urgent* job
`BinaryHeap` always pops the **largest** item. In `pool.rs`, `Ord for Queued` is written so that "larger" means "more urgent": a lower priority number, then the earlier submission. It's the same trick as passing a reversed `IComparer` to `PriorityQueue<TElement, TPriority>`.
