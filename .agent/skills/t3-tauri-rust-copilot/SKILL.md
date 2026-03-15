---
name: t3-tauri-rust-copilot
description: Use when implementing or reviewing gtaurus_lib changes that affect public APIs, traits, transport behavior, or Tauri/server compatibility.
---

# t3-tauri-rust-copilot

Workflow skill for developing `gtaurus_lib` safely with GitHub Copilot in a T3 + Tauri + Rust ecosystem.

## Use This Skill When

- Changing public structs/enums/traits.
- Modifying serial/tcp transport or timeout semantics.
- Updating parser/encoding behavior.
- Fixing interoperability issues between app/server/library layers.

## Required Workflow

1. Identify API/trait compatibility impact.
2. Keep transport and domain layers cleanly separated.
3. Prefer additive API changes over breaking changes.
4. Add tests for malformed input and timeout/error paths.
5. Run fmt, clippy, test, and check before handoff.

## Quality Gates

- Public API behavior is documented and predictable.
- Error mapping preserves root cause context.
- Transport behavior is deterministic and bounded.
- Regression tests exist for fixed bugs.
