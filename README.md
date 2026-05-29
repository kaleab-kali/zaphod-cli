# Zaphod CLI

Zaphod is a small Git workflow tool for developers who often work between two
related branches.

It remembers a branch pair, shows where you are, and switches to the other side
only when the repository is safe to touch. The goal is not to replace Git. The
goal is to make one repetitive workflow easier without hiding what is happening.

## Project Status

Zaphod is in early development. The current `0.1.x` CLI can:

- Detect the current Git repository.
- Pair two branches in repo-local metadata.
- Show the active pair status.
- Switch to the paired branch.
- Preview a safe switch without changing branches.
- List, rename, and remove branch pairs.
- Run preflight checks for humans and coding agents.
- Assert the expected branch, pair, or pair side before scripted work starts.
- Claim and release a pair/branch for an agent session.
- Emit a handoff snapshot for agent-to-agent continuation.
- Refuse unsafe switches when the worktree is dirty or Git is mid-operation.
- Emit JSON status for scripts.
- Diagnose repository, metadata, and branch-pair health in text or JSON.
- Generate shell completions.

Until Zaphod reaches `1.0`, command names and output may change between minor
versions.

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

## Agentic Coding

Zaphod is not a general Git replacement. For normal human Git users, it is a
small safety and convenience tool. Its stronger direction is agent-safe Git
workflow guardrails: giving humans and coding agents machine-readable checks,
safe branch-pair movement, and clear refusal behavior before touching
repository state.

Coding agents can use Zaphod as a preflight gate:

```sh
zaphod preflight --agent codex --json
zaphod preflight --agent codex --stale-after 2h --json
zaphod claim --agent codex --pair api --json
zaphod assert --pair api --side left --json
zaphod handoff --agent codex --json
zaphod status --json
zaphod doctor --json
zaphod switch --dry-run
```

This lets an agent check branch, worktree, merge, rebase, and pair state before
editing files or switching branches.

This matters most when an agent is operating from instructions such as "make
the API change on the backend branch, then update the UI branch." Without a
guard, the agent has to infer whether the current branch is the correct place
to edit. With Zaphod, the script can make that expectation explicit:

```sh
zaphod assert --pair search --side left --json
```

If the repository is on the UI side, outside the pair, detached, or missing the
expected pair, the command fails before any file edits happen. The agent can
then stop, report the mismatch, or ask for human approval instead of continuing
on the wrong branch.

When several agents, scripts, or terminals may touch the same repository,
claims add a lightweight coordination layer:

```sh
zaphod claim --agent codex --pair search --json
# work on the branch
zaphod claims --pair search --stale-after 2h --json
zaphod prune-claims --pair search --stale-after 2h --json
zaphod prune-claims --pair search --stale-after 2h --apply
zaphod unclaim --agent codex --pair search
zaphod unclaim --agent codex --pair search --branch feature/api
```

Claims are local metadata only. They do not lock Git, modify branches, or delete
anything. They make accidental overlap visible so another agent can refuse to
start on the same pair and branch.

Stale-claim filters help automation notice abandoned sessions after crashes,
terminal closes, or interrupted agent runs. They are read-only, so cleanup
still requires an explicit `unclaim` or `prune-claims --apply`.

For handoffs between agents or terminals, `zaphod handoff --json` captures the
current branch, selected pair status, active claims, and claim readiness in one
read-only report.

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

### GitHub Release Binaries

Download the latest binary for your platform from the
[GitHub Releases page](https://github.com/kaleab-kali/zaphod-cli/releases).

Linux:

```sh
curl -LO https://github.com/kaleab-kali/zaphod-cli/releases/latest/download/zaphod-linux
curl -LO https://github.com/kaleab-kali/zaphod-cli/releases/latest/download/zaphod-linux.sha256
sha256sum -c zaphod-linux.sha256
chmod +x zaphod-linux
sudo install -m 0755 zaphod-linux /usr/local/bin/zaphod
```

macOS:

```sh
curl -LO https://github.com/kaleab-kali/zaphod-cli/releases/latest/download/zaphod-macos
curl -LO https://github.com/kaleab-kali/zaphod-cli/releases/latest/download/zaphod-macos.sha256
shasum -a 256 -c zaphod-macos.sha256
chmod +x zaphod-macos
sudo install -m 0755 zaphod-macos /usr/local/bin/zaphod
```

Windows PowerShell:

```powershell
Invoke-WebRequest -Uri https://github.com/kaleab-kali/zaphod-cli/releases/latest/download/zaphod-windows.exe -OutFile zaphod-windows.exe
Invoke-WebRequest -Uri https://github.com/kaleab-kali/zaphod-cli/releases/latest/download/zaphod-windows.exe.sha256 -OutFile zaphod-windows.exe.sha256
$expected = (Get-Content .\zaphod-windows.exe.sha256).Split(' ')[0]
$actual = (Get-FileHash .\zaphod-windows.exe -Algorithm SHA256).Hash.ToLower()
if ($actual -ne $expected) { throw "checksum mismatch" }
New-Item -ItemType Directory -Force "$HOME\bin"
Move-Item .\zaphod-windows.exe "$HOME\bin\zaphod.exe"
```

Add `$HOME\bin` to your `PATH` if it is not already there.

Then confirm the binary is available:

```sh
zaphod --version
zaphod --help
```

### From Source

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
zaphod init feature/ui
zaphod status
zaphod switch
```

`init` uses the current branch as one side of the pair. You can also name both
branches explicitly:

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
Branch names are validated with Git's branch-name rules before Zaphod stores
the pair.

### `zaphod init <other>`

Store a branch pair using the current branch and another local branch:

```sh
zaphod init feature/ui
zaphod init feature/ui --name api
```

Both branches must already exist locally. `init` refuses detached HEADs, invalid
branch names, missing branches, and attempts to pair the current branch with
itself.

### `zaphod preflight`

Check whether the current repository is ready for paired-branch work:

```sh
zaphod preflight --json
zaphod preflight --agent codex --json
zaphod preflight --agent codex --stale-after 2h --json
```

Preflight is read-only. It reports the requested pair, current branch, paired
target branch, worktree state, Git operation state, switch readiness, and any
refusal reasons. It exits successfully when the pair is ready and exits with an
error when the repository is not ready.

Use `--agent` to check whether another agent has already claimed the current
pair and branch. This does not create or remove a claim; it only reports whether
claiming would be allowed.

Use `--stale-after` with `--agent` to mark old claim conflicts in the preflight
report. Preflight still refuses the conflict; stale reporting is only a signal
for scripts or humans to decide whether an explicit `unclaim` is appropriate.

### `zaphod assert`

Fail fast when the current repository state does not match an expected branch,
pair, or pair side:

```sh
zaphod assert --branch feature/api
zaphod assert --pair api
zaphod assert --side left
zaphod assert --pair api --side right --json
```

If no selector is provided, `assert` checks that the current branch belongs to
the default pair. `--side` uses the default pair unless `--pair` is provided.

This command is read-only and is designed for scripts and coding agents that
need to prove they are in the right place before editing files.

### `zaphod claim`

Claim the current pair and branch for an agent session:

```sh
zaphod claim --agent codex --pair api
zaphod claim --agent codex --pair api --json
```

`claim` writes repo-local metadata under `.git/zaphod/claims.toml`. It refuses
when another agent has already claimed the same pair and current branch. It also
uses the same readiness checks as `preflight`, so it refuses when the worktree
is dirty, Git is mid-merge or mid-rebase, the current branch is outside the
pair, or the paired target branch is missing.

Agent names may contain only letters, numbers, `.`, `_`, and `-`.

### `zaphod claims`

List active agent session claims:

```sh
zaphod claims
zaphod claims --json
zaphod claims --agent codex --pair api --branch feature/api --json
zaphod claims --pair api --stale-after 2h --json
```

Use filters when a script needs to check a specific agent, pair, branch, or
stale-claim window without parsing unrelated claim entries. Durations use a
positive number followed by `s`, `m`, `h`, or `d`.

### `zaphod prune-claims`

Preview or remove stale agent session claims:

```sh
zaphod prune-claims --stale-after 2h
zaphod prune-claims --pair api --stale-after 2h --json
zaphod prune-claims --pair api --stale-after 2h --apply
```

`prune-claims` defaults to dry-run mode and does not change metadata unless
`--apply` is present. Use `--agent`, `--pair`, and `--branch` to narrow the
cleanup scope. The command only edits `.git/zaphod/claims.toml`; it never
switches branches, deletes files, or changes Git history.

### `zaphod unclaim`

Release an agent session claim for the current pair and branch:

```sh
zaphod unclaim --agent codex --pair api
zaphod unclaim --agent codex --pair api --json
zaphod unclaim --agent codex --pair api --branch feature/api
```

By default, `unclaim` releases the matching claim for the current branch. Use
`--branch` to release a claim for another branch without switching to it. This
is useful when an agent stopped early and left stale claim metadata behind.

`unclaim` only removes the matching claim metadata. It does not switch branches,
clean files, or alter Git history.

### `zaphod handoff`

Emit a read-only snapshot for another agent, script, or terminal to continue
from:

```sh
zaphod handoff
zaphod handoff --json
zaphod handoff --name api --agent codex --json
```

The handoff report includes repository root, current branch, worktree state,
Git operation state, selected pair status, active claims, and optional claim
readiness for the requested agent. It does not create claims, remove claims, or
switch branches.

### `zaphod status`

Show the active pair status:

```sh
zaphod status
```

To inspect every configured pair in the repository, use `--all`:

```sh
zaphod status --all
```

For scripts, use JSON output:

```sh
zaphod status --json
zaphod status --all --json
```

JSON fields use script-friendly names:

```json
{
  "pair": "default",
  "current": "feature/api",
  "other": "feature/ui",
  "worktree": "clean",
  "git_state": "ready",
  "switch_allowed": true,
  "refusal_reasons": []
}
```

When switching is refused, `switch_allowed` is `false` and
`refusal_reasons` contains one or more values:

```json
{
  "pair": "default",
  "current": "feature/api",
  "other": "feature/ui",
  "worktree": "dirty",
  "git_state": "ready",
  "switch_allowed": false,
  "refusal_reasons": ["dirty_worktree"]
}
```

Known refusal reasons are `dirty_worktree`, `merge_in_progress`,
`rebase_in_progress`, and `target_branch_missing`.

`status --all --json` returns an array. Each item includes the pair's `left`
and `right` branch names, whether the pair is `active` for the current branch,
branch existence booleans, and switch availability details. Inactive pairs use
`current_branch_not_paired` as the refusal reason.

### `zaphod switch`

Switch to the other branch in the pair:

```sh
zaphod switch
```

Zaphod refuses to switch if the worktree is dirty, a merge is in progress, or a
rebase is in progress. It also refuses when the paired target branch no longer
exists. Use `zaphod status` to see the current refusal reason before switching.

To preview the target without changing branches, use `--dry-run`:

```sh
zaphod switch --dry-run
```

Dry-run mode applies the same safety checks as a real switch.

### `zaphod list`

List all branch pairs configured for the current repository:

```sh
zaphod list
```

For scripts, use JSON output:

```sh
zaphod list --json
```

### `zaphod rename <old> <new>`

Rename a branch pair label without changing either Git branch:

```sh
zaphod rename default api
```

Zaphod refuses to overwrite an existing pair name. Pair names must contain only
letters, numbers, `.`, `_`, and `-`.

### `zaphod doctor`

Diagnose Git availability, repository state, metadata health, and configured
branch pairs:

```sh
zaphod doctor
```

`doctor` is read-only. It exits successfully when the repository and configured
pairs look healthy, and exits with an error when it finds problems such as
corrupt metadata or missing paired branches.

For scripts, use JSON output:

```sh
zaphod doctor --json
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

## Exit Codes

Zaphod uses stable app-level exit codes for scripts:

```text
0  success
1  runtime or unexpected failure
2  invalid input, missing branch, missing pair, or incompatible repository state
3  safety refusal, such as a dirty worktree or in-progress merge
4  doctor found repository, metadata, or pair health problems
```

Use `--json-errors` to emit app-level failures as JSON on stderr:

```sh
zaphod --json-errors switch
```

Example error output:

```json
{"error":{"exit_code":3,"kind":"switch_refused","message":"refusing to switch: worktree has uncommitted changes"}}
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

Metadata saves use atomic file replacement so interrupted writes do not leave
`.git/zaphod/pairs.toml` partially written.

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

The release workflow builds binary artifacts for Linux, macOS, and Windows and
publishes them as GitHub Release assets when a `v*` tag is pushed. Each binary
is published with a matching `.sha256` checksum file:

```sh
git tag -a v0.1.0 -m "v0.1.0"
git push origin v0.1.0
```

Verify downloaded release assets before running them:

```sh
sha256sum -c zaphod-linux.sha256
shasum -a 256 -c zaphod-macos.sha256
```

On Windows PowerShell:

```powershell
Get-FileHash .\zaphod-windows.exe -Algorithm SHA256
Get-Content .\zaphod-windows.exe.sha256
```

Pull requests that touch release-critical files validate the release build
without publishing a GitHub Release.

Before tagging a release, run the full quality gate and manually test the
release binary in a temporary Git repository.

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
