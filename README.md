# Zaphod CLI

Zaphod is a small Git workflow tool for developers who often work between two
related branches.

It remembers a branch pair, shows where you are, and switches to the other side
only when the repository is safe to touch. The goal is not to replace Git. The
goal is to make one repetitive workflow easier without hiding what is happening.

## Project Status

Zaphod is in early development. The current CLI can:

- Detect the current Git repository.
- Pair two branches in repo-local metadata.
- Show the active pair status.
- Switch to the paired branch.
- List and remove branch pairs.
- Refuse unsafe switches when the worktree is dirty or Git is mid-operation.
- Emit JSON status for scripts.

Until `v0.1` is tagged, command names and output may change.

## Why Zaphod?

Git is powerful, but switching between two related branches can become noisy:

- `main` and a feature branch.
- A backend branch and a frontend branch.
- A review branch and an implementation branch.
- Two worktrees used for the same task.

Zaphod keeps that relationship explicit:

```text
feature/api  <->  feature/ui
```

Then it gives you a small set of commands for checking and moving between the
two sides safely.

## Safety Model

Zaphod is intentionally cautious.

The CLI should refuse operations that could surprise the user or disturb local
work. In particular, switching should fail when:

- The current directory is not inside a Git repository.
- The worktree has uncommitted changes.
- Git is in the middle of a merge or rebase.
- The current HEAD is detached.
- The current branch is not part of a known pair.
- The target branch cannot be found.

Forceful behavior may be added later, but the first release prioritizes clear
refusals over convenience.

## Installation

Zaphod is not published to crates.io yet. Install from a local checkout:

```sh
cargo install --path .
```

Then confirm the binary is available:

```sh
zaphod --help
```

## Quickstart

Inside a Git repository with two existing local branches:

```sh
zaphod pair feature/api feature/ui
zaphod status
zaphod switch
```

Example status output:

```text
Pair: default
Current: feature/api
Other: feature/ui
Worktree: clean
Git state: ready
Switch: allowed
```

If the worktree is dirty, switching is refused:

```text
Pair: default
Current: feature/api
Other: feature/ui
Worktree: dirty
Git state: ready
Switch: refused (worktree has uncommitted changes)
```

## Commands

### `zaphod pair <left> <right>`

Store a branch pair in the current repository:

```sh
zaphod pair feature/api feature/ui
```

Use `--name` to store more than one pair:

```sh
zaphod pair main feature/api --name api
```

Both branches must already exist locally.

### `zaphod status`

Show the active pair status:

```sh
zaphod status
```

For scripts, use JSON output:

```sh
zaphod status --json
```

### `zaphod switch`

Switch to the other branch in the pair:

```sh
zaphod switch
```

Zaphod refuses to switch if the worktree is dirty, a merge is in progress, or a
rebase is in progress.

### `zaphod list`

List all branch pairs configured for the current repository:

```sh
zaphod list
```

### `zaphod unpair`

Remove a branch pair:

```sh
zaphod unpair
```

Use `--name` to remove a named pair:

```sh
zaphod unpair --name api
```

## Demo Transcript

```text
$ git branch --show-current
feature/api

$ zaphod pair feature/api feature/ui
Paired 'default': feature/api <-> feature/ui

$ zaphod status
Pair: default
Current: feature/api
Other: feature/ui
Worktree: clean
Git state: ready
Switch: allowed

$ zaphod switch
Switched pair 'default': feature/api -> feature/ui
```

## Metadata

Zaphod stores branch pair data inside the current repository under `.git/zaphod`.

This keeps pair metadata local to the repository and avoids changing global Git
configuration. The metadata format is TOML so it stays readable and easy to
debug.

## Development

Zaphod is a Rust CLI using:

- `clap` for command parsing.
- `serde`, `serde_json`, and TOML for metadata and machine-readable output.
- Carefully wrapped `git` commands for repository operations.

The implementation is organized around three boundaries:

- CLI parsing and command output.
- Core branch-pair and safety logic.
- Git repository adapter.

Run the local quality gate before opening a pull request:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

## Contributing

Contributions are welcome. The most useful contributions are:

- Clear bug reports.
- Small pull requests with tests.
- Documentation improvements.
- Safety-focused edge cases around Git state.

Before opening a pull request, please make sure the project formats, lints, and
tests cleanly.

## License

Zaphod is licensed under the MIT License. See [LICENSE](LICENSE).
