---
applyTo: "**/*.rs"
description: Best practices for using GitHub Copilot in gtaurus_lib (Rust library for T3 + Tauri ecosystem).
---

# Copilot Instructions: T3 + Tauri + Rust Library

## Design Principles

- Keep public APIs stable, explicit, and strongly typed.
- Keep domain logic independent from transport details.
- Treat parser and protocol boundaries as high-risk correctness surfaces.

## Coding Expectations

- Prefer explicit structs/enums over untyped maps/strings.
- Keep trait contracts minimal and test-friendly.
- Avoid hidden retries, implicit timeouts, and panic paths.
- Keep allocation-heavy work out of hot paths when practical.

## Contract and Error Rules

- Prefer additive API evolution; document any breakage clearly.
- Use structured errors and preserve lower-level context.
- Keep timeout and retry semantics explicit and consistent.
- Validate and reject malformed frames safely.

## Verification Rules

- Add regression tests for every fixed bug.
- Cover parser, transport, and trait conformance paths.
- Run `cargo fmt`, `cargo clippy`, `cargo test`, and `cargo check` before finalizing.
