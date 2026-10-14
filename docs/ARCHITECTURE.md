# Kinora architecture

> Phase 0 snapshot: the crates exist, but only `kinora_app` contains code so far (an empty window).
> The threading and data flow below is the **design** from PROJECT_BRIEF.md §7, and is filled in phase by phase.

## Crates

```mermaid
graph TD
    app[kinora_app<br/>binary · Slint UI · wiring]
    core[kinora_core<br/>pure logic, no I/O]
    decode[kinora_decode<br/>sniffing · decoding]
    cache[kinora_cache<br/>byte-budgeted LRU]
    video[kinora_video<br/>libmpv FFI · backends A/B/C]
    edit[kinora_edit<br/>edit pipeline · atomic save]
    platform[kinora_platform<br/>OS integration · hardware]

    app --> core & decode & cache & video & edit & platform
    decode --> core
    cache --> core & decode
    edit --> core & decode
    video --> core
    platform --> core
```

Rules: `kinora_core` depends on no other Kinora crate, and there are no cycles. Platform-specific code lives only in `kinora_platform`, plus the GL parts of `kinora_video`. `unsafe` is allowed only in those two crates. The arrows between library crates are planned and get added as real code needs them.

## Threading model (planned)

```mermaid
graph LR
    UI[UI thread<br/>Slint event loop] -- jobs with priority + generation id --> Pool[decode pool<br/>N-1 workers]
    Pool -- results via slint::invoke_from_event_loop --> UI
    UI -- commands --> MPV[libmpv<br/>one lazy instance]
    MPV -- typed events --> UI
    Scan[directory scan thread] -- sibling list --> UI
```

- The UI thread never blocks on disk, decoding or mpv.
- When the user navigates, stale decode jobs are dropped before they start (by generation id).
- Pool sizes, cache budgets and preload distances all come from the hardware profile (Lite/Typical/High).
