# Development Workflow Rules

## Copilot-First Loop

1. Identify the public API surface affected.
2. Read trait and transport call sites before editing.
3. Implement focused changes with minimal contract churn.
4. Add/adjust unit and integration tests.
5. Validate and summarize impact to downstream crates.

## Validation Commands

Run after significant changes:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo check
```

## Library Quality Checklist

- Public types/traits remain coherent and documented.
- Timeout/retry behavior is explicit and deterministic.
- Parser/transport failures are safe and actionable.
- No new panic paths in runtime logic.
- Regression tests added for bug fixes.

## Git Practice

- Do not commit unless explicitly requested.
- Keep API-impacting changes isolated and documented.
- Check remote state (`git fetch`) before larger edits.
