# GitHub Workflow Rules

## Pull Request Quality

- Keep PR scope focused (types, transport, parser, or driver concern).
- Clearly describe semver/API impact.
- Include migration notes when behavior or signatures change.
- Provide test evidence for success and failure paths.

## CI Expectations

A library PR should pass:

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test`
4. `cargo check`

## Copilot Review Guidance

- Use Copilot review to surface broad concerns quickly.
- Require human review for public API and protocol changes.
- Every bug-fix PR should include a deterministic regression test.
