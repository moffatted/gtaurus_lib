# Skills Guide for gtaurus_lib

## Note

This file is a human-facing engineering guide.

Agent-loadable skills should be defined as `SKILL.md` files under
`.agent/skills/<skill-name>/SKILL.md`.

## Purpose

This file defines engineering skills and quality standards for the Rust library in this workspace.
The focus is reusable, stable, testable protocol and transport abstractions.

## Core Skill Areas

### 1. Library API Stability

- Treat public types and trait signatures as stable contracts.
- Prefer additive changes over breaking changes.
- Mark deprecations clearly before removal.
- Keep behavior predictable across patch releases.

### 2. Type Design and Domain Modeling

- Use small, strongly typed domain objects instead of loosely typed maps.
- Prefer enums for finite protocol states and command categories.
- Keep conversion boundaries explicit with TryFrom and From where appropriate.
- Avoid exposing internal transport details in public-facing types.

### 3. Error Handling

- Use structured error types with clear categories (io, protocol, validation, timeout).
- Preserve original context when mapping lower-level errors.
- Return Result consistently and avoid panic paths in library code.
- Keep error messages actionable for callers.

### 4. Transport Abstraction (serial, tcp, mock)

- Keep transport concerns behind traits in the transport module.
- Ensure every transport implementation follows consistent timeout semantics.
- Isolate framing, parsing, and retry logic from connection management.
- Design interfaces so callers can test without hardware dependencies.

### 5. Concurrency and Safety

- Keep shared mutable state minimal and explicit.
- Prefer ownership and borrowing clarity over convenience clones.
- Document thread-safety assumptions for any shared components.
- Use bounded retries and cancellation-aware loops for IO workflows.

### 6. Testing Strategy

- Unit tests for parsers, validators, and pure transforms.
- Trait-level tests to enforce behavior consistency across transports.
- Integration tests for realistic command/response workflows.
- Regression tests for every fixed bug, especially protocol edge cases.

### 7. Performance and Resource Discipline

- Avoid unnecessary allocations in hot parsing/encoding paths.
- Reuse buffers where practical and safe.
- Keep read/write loops bounded with explicit timeouts.
- Measure before optimizing; record rationale for non-obvious optimizations.

### 8. Documentation Quality

- Add rustdoc for all public structs, enums, traits, and functions.
- Include examples for common usage and failure handling.
- Document protocol assumptions and transport limitations.
- Keep README usage examples aligned with current APIs.

### 9. Source Control and GitHub Workflow

- Keep public API changes isolated in focused pull requests.
- Include semver impact notes for any public contract changes.
- Require changelog entries for behavioral or API-impacting updates.
- Request review from maintainers owning transport and type boundaries.
- Require CI to run unit, integration, and lint checks before merge.
- Add migration guidance in pull requests when deprecating APIs.

### 10. T3 + Tauri + Rust Library Testing Practices

- Validate library behavior with tests that mirror real app command sequences.
- Keep protocol parser tests table-driven with edge-case coverage.
- Add contract tests to ensure stable behavior for frontend expectations.
- Use transport conformance tests shared across serial and tcp implementations.
- Include fuzz-like malformed frame tests for parser hardening.
- Verify timeout and retry semantics with deterministic fake clocks where possible.
- Add regression tests for every API or protocol bug fixed in production.

## Coding Standards

- Prefer explicit names over short abbreviations.
- Keep modules focused: types, traits, transport, driver responsibilities stay separated.
- Avoid mixing refactors with behavior changes in the same commit.
- Use clippy-friendly patterns and fix warnings before merge when practical.
- After creating or editing any .md file, run markdownlint CLI and fix all warnings.

## Definition of Done Checklist

- Public API impact reviewed and documented.
- Errors are structured and propagated with context.
- Unit and integration tests are added or updated.
- No new panic paths in library runtime code.
- rustdoc and README examples remain accurate.
- Formatting, linting, and tests pass.

## Pre-Merge Review Heuristics

- Can this change be used safely by external callers without hidden assumptions?
- Are transport-specific behaviors kept out of domain interfaces?
- Is timeout/retry behavior deterministic and testable?
- Are protocol parse failures and partial frames handled safely?
- Is there at least one test proving the bug fix or new behavior?
