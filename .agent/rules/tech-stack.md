---
trigger: always_on
---

# Tech Stack

## Runtime

- Rust stable via Cargo.
- Library crate consumed by Tauri app and server components.

## Crate Structure

- `src/lib.rs`: crate exports and module wiring.
- `src/types.rs`: shared domain types.
- `src/traits.rs`: behavior abstraction interfaces.
- `src/transport/*`: serial/tcp transport implementations.
- `src/driver.rs`: domain operations over transport abstractions.

## Integration Context

- Public API stability is critical for dependent crates/app layers.
- Keep library behavior deterministic and testable without hardware.

## Engineering Direction

- Prefer strongly typed API boundaries.
- Isolate transport concerns behind traits.
- Preserve semver-friendly, additive public API evolution.
- Use structured errors and explicit timeout semantics.
