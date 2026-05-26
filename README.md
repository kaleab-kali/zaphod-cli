# Zaphod CLI

Zaphod is a small Git workflow tool for developers who often work between two
related branches.

It remembers a branch pair, shows where you are, and switches to the other side
only when the repository is safe to touch. The goal is not to replace Git. The
goal is to make one repetitive workflow easier without hiding what is happening.

## Project Status

Zaphod is in early development. The first public milestone is a conservative
`v0.1` CLI that can:

- Detect the current Git repository.
- Pair two branches in repo-local metadata.
- Show the active pair status.
- Switch to the paired branch.
- Refuse unsafe switches when the worktree is dirty or Git is mid-operation.

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

## Planned Usage

The intended `v0.1` workflow looks like this:

```sh
zaphod pair feature/api feature/ui
zaphod status
zaphod switch
```

Additional planned commands:

```sh
zaphod list
zaphod unpair
zaphod status --json
```

Example status output:

```text
Pair: default
Current: feature/api
Other: feature/ui
Worktree: clean
Switch: allowed
```

## Metadata

Zaphod stores branch pair data inside the current repository under `.git/zaphod`.

This keeps pair metadata local to the repository and avoids changing global Git
configuration. The metadata format is planned to be TOML so it can stay readable
and easy to debug.

## Installation

Installation instructions will be added once the first runnable version exists.

Planned source install:

```sh
cargo install --path .
```

Future releases may include prebuilt binaries.

## Development

Zaphod is planned as a Rust CLI using:

- `clap` for command parsing.
- `serde` and TOML for metadata.
- Carefully wrapped `git` commands for repository operations.

The implementation is organized around three boundaries:

- CLI parsing and command output.
- Core branch-pair and safety logic.
- Git repository adapter.

## Contributing

Contributions are welcome once the initial scaffold is in place.

For now, the most useful contributions are:

- Clear bug reports.
- Small pull requests with tests.
- Documentation improvements.
- Safety-focused edge cases around Git state.

Before opening a pull request, please make sure the project formats, lints, and
tests cleanly.

## License

Zaphod is licensed under the MIT License. See [LICENSE](LICENSE).
