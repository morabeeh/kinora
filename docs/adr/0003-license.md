# ADR 0003 — Project license: GPL-3.0-or-later

- **Status:** Accepted (2026-10-14)
- **Deciders:** project owner

## Context
Kinora links to components with copyleft licenses:
- **Slint** is offered as `GPL-3.0-only OR` two proprietary Slint licenses. The royalty-free option has conditions (attribution, desktop/mobile only) that we'd rather not depend on.
- **libmpv** is LGPL-2.1+ or GPL-2.0+, depending on how it was built. Many distribution builds are GPL.
- **libheif** is LGPL-3.0 and **libde265** is LGPL-3.0.

## Decision
License Kinora under **GPL-3.0-or-later**. We use Slint under its GPL-3.0 option.

## Why this is compatible
- Our code is "GPL-3.0 or later". Combined with Slint's GPL-3.0-only code, the distributed program as a whole is under GPL-3.0, which is allowed: "or later" includes 3.0.
- GPL-2.0-*or-later* libmpv builds can be used under GPL-3.0. LGPL libraries are compatible when **dynamically linked** and shipped with their license texts and a source offer.
- Permissive dependencies (MIT, Apache-2.0, BSD, ISC, Zlib, MPL-2.0, Unicode, CC0) are compatible.

## Consequences
- `deny.toml` allows only GPL-3.0-compatible licenses, and CI fails on anything else, on unknown licenses, on yanked crates and on security advisories.
- `THIRD_PARTY_NOTICES.md` is generated with cargo-about and ships in every package.
- LGPL C libraries are **never statically linked**. Windows/macOS bundles include their license texts and say where to get the exact sources.
- Choosing a different license later would mean re-checking every dependency, especially Slint.
