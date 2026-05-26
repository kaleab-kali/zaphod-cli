# Agent Instructions

These instructions apply to automation and coding agents working in this
repository.

## Repository Workflow

- Use small, focused branches named after the functionality or documentation
  they change, for example `feat/git-repository-detection` or
  `docs/agent-collaboration-guidelines`.
- Use professional, functionality-specific pull request titles and commit
  messages. Avoid phase-only names such as `phase-1` or vague names such as
  `updates`.
- Open pull requests for meaningful changes. Keep each pull request reviewable
  and scoped to one behavior, maintenance task, or documentation task.
- Do not squash merge pull requests. Use normal merge commits unless the
  maintainer explicitly requests a different strategy.
- Keep progress committed cleanly. Each commit should build toward an
  open-source-ready project state.

## Commit Hygiene

- Do not add Claude, AI, or co-author trailers to commits.
- Do not include generated attribution footers unless the maintainer explicitly
  requests them.
- Prefer conventional-style commit subjects when they fit the change:
  - `feat: add CLI command parser`
  - `fix: refuse switch during rebase`
  - `docs: explain safety model`
  - `test: cover dirty worktree refusal`

## Engineering Standards

- Favor clean, readable, professional code over clever shortcuts.
- Keep safety boundaries explicit, especially around Git operations.
- Avoid unsafe Rust. Only use `unsafe` in rare cases where it is genuinely
  necessary, carefully justified, and covered by tests.
- Run the local quality gate before opening or merging pull requests:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test --all-targets --all-features`

## Product Direction

- Zaphod is a cautious Git workflow CLI for paired branches.
- The README should describe the public project, not internal planning notes.
- Prioritize safe refusals over surprising behavior.
- Do not implement destructive Git behavior unless it is explicitly requested,
  guarded, documented, and tested.
