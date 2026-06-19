# Changelog

All notable changes to Zaphod will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses semantic versioning for published releases.

## Unreleased

## 0.1.36 - 2026-06-19

### Added

- `prune-claims --target` for dry-run-by-default stale claim cleanup scoped to
  the paired target branch without switching branches.

## 0.1.35 - 2026-06-19

### Added

- `claims --target` for listing claims on the paired target branch without
  switching branches.

## 0.1.34 - 2026-06-19

### Added

- `claim --clear-note` and `heartbeat --clear-note` for removing claim notes
  without releasing the underlying agent claim.

## 0.1.33 - 2026-06-06

### Added

- `claim --note` and `heartbeat --note` for attaching short local context to
  agent claims while preserving notes across refreshes.

## 0.1.32 - 2026-06-05

### Added

- `assert --agent` and `assert --agent --require-claim` for read-only claim
  ownership checks on the current pair and branch, including while the worktree
  is dirty.

## 0.1.31 - 2026-06-05

### Added

- `switch --agent` and `switch --agent --require-claim` for refusing agent
  switches into target branches claimed by another agent or not yet claimed by
  the requested agent.

## 0.1.30 - 2026-06-05

### Added

- `handoff --require-claim` for read-only continuation snapshots that refuse
  unless the requested agent already owns the current pair and branch claim.

## 0.1.29 - 2026-06-05

### Added

- `preflight --require-claim` for read-only resume checks that refuse unless
  the requested agent already owns the current pair and branch claim.

## 0.1.28 - 2026-06-04

### Added

- `claims --conflicts-for` for read-only claim summaries scoped to claims owned
  by other agents before an agent starts or resumes work.

## 0.1.27 - 2026-06-04

### Added

- `heartbeat --stale-after` for reporting whether a conflicting claim is stale
  without taking it over or mutating existing claim metadata.

## 0.1.26 - 2026-06-04

### Added

- `claim --stale-after` for reporting whether a conflicting claim is stale
  without taking it over or mutating existing claim metadata.

## 0.1.25 - 2026-06-04

### Added

- `prune-claims --side` for dry-run-by-default claim cleanup scoped to the left
  or right side of a named pair without switching branches.

## 0.1.24 - 2026-06-04

### Added

- `claims --side` for read-only claim reports scoped to the left or right side
  of a named pair without switching branches.

## 0.1.23 - 2026-06-03

### Added

- `unclaim --side` for releasing claims by the left or right side of a named
  pair without switching branches.

## 0.1.22 - 2026-06-03

### Added

- `prune-claims --current` for dry-run-by-default cleanup scoped to the current
  Git branch.

## 0.1.21 - 2026-06-03

### Added

- `claims --current` for filtering claim reports to the current Git branch
  without scripting a separate branch lookup.

## 0.1.20 - 2026-06-03

### Added

- `handoff --branch` and `handoff --side` for guarded agent handoff snapshots
  that refuse when the current branch or pair side is not the expected one.

## 0.1.19 - 2026-06-03

### Added

- `heartbeat --branch` and `heartbeat --side` for guarded claim refreshes that
  first verify the current branch or pair side.

## 0.1.18 - 2026-06-03

### Added

- `claim --branch` and `claim --side` for guarded claim writes that first verify
  the current branch or pair side.

## 0.1.17 - 2026-06-01

### Added

- `preflight --branch` and `preflight --side` for read-only branch expectation
  checks before agentic or scripted work starts.

## 0.1.16 - 2026-06-01

### Added

- `preflight --agent` and agent handoff reports now include metadata lock state
  and refuse claim readiness when pair or claim metadata writes are already
  locked.

## 0.1.15 - 2026-06-01

### Added

- `doctor` now reports whether the repo-local metadata mutation lock is clear
  or currently present.

## 0.1.14 - 2026-06-01

### Added

- Repo-local metadata mutation locking to avoid overlapping pair or claim
  writes from concurrent Zaphod processes.

## 0.1.13 - 2026-05-30

### Added

- Explicit schema-version validation for Zaphod pair and claim metadata.

## 0.1.12 - 2026-05-30

### Added

- `prune-claims --orphaned` for dry-run-by-default cleanup of claim metadata
  that references removed pairs or unavailable branches.

## 0.1.11 - 2026-05-30

### Added

- `doctor` now reports agent claims that reference missing pairs, branches that
  are no longer part of their pair, or local branches that no longer exist.

## 0.1.10 - 2026-05-30

### Added

- `pair --json`, `init --json`, `rename --json`, and `unpair --json` for
  machine-readable pair metadata mutation reports.

## 0.1.9 - 2026-05-30

### Added

- `switch --json` for machine-readable switch success, dry-run, and safety
  refusal reports.

## 0.1.8 - 2026-05-30

### Added

- `handoff --stale-after` for marking old claim conflicts in agent handoff
  snapshots.

## 0.1.7 - 2026-05-30

### Added

- `heartbeat` command for refreshing an existing agent claim during long-running
  sessions.
- `doctor --stale-after` for read-only stale claim health checks.

## 0.1.6 - 2026-05-30

### Added

- `init` command for pairing the current branch with another local branch.

## 0.1.5 - 2026-05-30

### Added

- `prune-claims` command for dry-run-by-default cleanup of stale agent claims,
  with metadata changes guarded behind `--apply`.

## 0.1.4 - 2026-05-29

### Added

- `claims --agent`, `claims --pair`, and `claims --branch` filters for
  script-friendly agent claim lookups.
- `claims --stale-after` for read-only stale agent claim detection.
- `preflight --stale-after` for reporting stale claim conflicts without
  automatically overriding them.
- `handoff` command for read-only agent continuation snapshots with pair,
  worktree, Git state, and claim details.
- `unclaim --branch` for releasing stale agent claims without switching
  branches.

## 0.1.3 - 2026-05-29

### Added

- `assert` command for read-only branch, pair, and pair-side checks before
  scripted or agentic work starts.
- `claim`, `claims`, and `unclaim` commands for lightweight local coordination
  between agent sessions working on the same pair and branch.
- `preflight --json` for read-only agent readiness checks before paired-branch
  work.
- `preflight --agent` for read-only claim conflict checks before an agent
  starts work.

## 0.1.2 - 2026-05-29

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
