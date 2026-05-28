# Changelog

All notable changes to Zaphod will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project intends to follow semantic versioning once releases begin.

## Unreleased

### Added

- Initial Rust binary scaffold.
- Public project README.
- Open source maintenance files.
- Branch pair metadata stored under `.git/zaphod/pairs.toml`.
- `pair`, `list`, and `unpair` commands.
- `status` command with plain text and JSON output.
- `switch` command with dirty-worktree and in-progress Git operation refusals.
- `completions` command for shell completion generation.
- Integration tests that exercise real temporary Git repositories.
- README workflow guidance, pair naming guidance, and release build notes.
- GitHub Actions workflow for release binary artifacts.
- GitHub Release publishing for tagged release binary artifacts.

### Changed

- README documentation now shows the `status --json` shape and known refusal
  reason values for script users.

### Fixed

- `status` now reports a missing paired target branch as a switch refusal,
  matching the behavior of `switch`.
