## Summary

- What changed?
- Why is it needed?

## Safety impact

- [ ] Does not run destructive Git operations.
- [ ] Preserves local work when refusing an operation.
- [ ] Updates refusal messaging or docs when user-facing behavior changes.

## Testing

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --all-features`

## Notes for reviewers

- Any Git state, branch, or worktree edge cases worth checking:
