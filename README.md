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
- Generate shell completions.

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

## Workflows

### Solo Development

Pair your main integration branch with the branch you are actively changing:

```sh
zaphod pair main feature/search
zaphod status
zaphod switch
```

This keeps the relationship explicit while still letting Git own the actual
branch state.

### Split Work

Pair two branches that represent different sides of the same task:

```sh
zaphod pair feature/api feature/ui --name search
zaphod status --name search
zaphod switch --name search
```

This is useful when backend and frontend changes move together, but should stay
reviewable as separate branches.

### Review Work

Pair an implementation branch with a review or experiment branch:

```sh
zaphod pair feature/parser review/parser-notes --name parser-review
```

Use `zaphod list` when a repository has multiple named pairs.

## Pair Naming

Pair names are local labels stored in the repository metadata. Names may contain
letters, numbers, `.`, `_`, and `-`.

Good names are short and tied to the workflow:

```text
default
api
search-ui
parser-review
```

Avoid names that depend on one person's machine or temporary context.

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

### `zaphod completions <shell>`

Generate shell completions to stdout:

```sh
zaphod completions bash
zaphod completions zsh
zaphod completions fish
zaphod completions powershell
zaphod completions elvish
```

Redirect the output to the location expected by your shell.

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

Build a release binary locally:

```sh
cargo build --release
```

The binary will be written to `target/release/zaphod` on Unix-like systems and
`target/release/zaphod.exe` on Windows.

Release packaging is not automated yet. Before tagging a release, run the full
quality gate and manually test the release binary in a temporary Git repository.

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
