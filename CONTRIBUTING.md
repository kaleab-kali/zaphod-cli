# Contributing

Thanks for your interest in contributing to Zaphod.

Zaphod is early-stage software, so small, focused pull requests are easiest to
review and merge. Safety-related changes should include tests that prove the CLI
does not disturb user work unexpectedly.

## Development Setup

Install the stable Rust toolchain, then run:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

## Pull Request Guidelines

- Keep pull requests focused on one behavior or maintenance task.
- Add or update tests for behavior changes.
- Update documentation when user-facing behavior changes.
- Prefer clear error messages over clever control flow.
- Avoid unsafe Git operations unless they are explicitly guarded and tested.

## Commit Messages

Use short conventional-style commit messages where practical:

```text
feat: add branch pair metadata model
fix: refuse switch during rebase
docs: explain safety model
test: cover dirty worktree switch refusal
```
