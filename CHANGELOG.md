# Changelog

All notable changes to Zaphod will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project intends to follow semantic versioning once releases begin.

## Unreleased

### Added

- `doctor --json` for structured repository and metadata health reports.
- `switch --dry-run` for previewing the target branch without switching.

## 0.1.1 - 2026-05-28

### Added

- `rename` command for changing branch-pair labels without changing Git
  branches.
- `status --all` output for auditing every configured branch pair.
- SHA-256 checksum files for GitHub Release binary artifacts.

## 0.1.0 - 2026-05-28

### Added

- Initial Rust binary scaffold.
- Public project README.
- Open source maintenance files.
- Branch pair metadata stored under `.git/zaphod/pairs.toml`.
- `pair`, `list`, and `unpair` commands.
- `status` command with plain text and JSON output.
- JSON output for `list`.
- `switch` command with dirty-worktree and in-progress Git operation refusals.
- `doctor` command for read-only repository and metadata diagnostics.
- `completions` command for shell completion generation.
- Integration tests that exercise real temporary Git repositories.
- README workflow guidance, pair naming guidance, and release build notes.
- GitHub Actions workflow for release binary artifacts.
- GitHub Release publishing for tagged release binary artifacts.
- Stable app-level exit codes for scripting.
- `--json-errors` flag for machine-readable app-level errors on stderr.

### Changed

- Metadata saves now use atomic file replacement to avoid partially written
  pair metadata.
- `pair` now validates branch names with Git's branch-name rules before looking
  up or storing a pair.
- README documentation now shows the `status --json` shape and known refusal
  reason values for script users.

### Fixed

- `status` now reports a missing paired target branch as a switch refusal,
  matching the behavior of `switch`.
