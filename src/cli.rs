use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

#[derive(Debug, Parser)]
#[command(
    name = "zaphod",
    version,
    about = "A cautious Git workflow CLI for paired branches.",
    long_about = None
)]
pub struct Cli {
    /// Emit machine-readable JSON for app-level errors.
    #[arg(long, global = true)]
    pub json_errors: bool,

    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum CliCommand {
    /// Pair two branches in the current repository.
    Pair {
        /// First branch in the pair.
        left: String,

        /// Second branch in the pair.
        right: String,

        /// Pair name.
        #[arg(long, default_value = "default")]
        name: String,
    },

    /// Pair the current branch with another local branch.
    Init {
        /// Other branch to pair with the current branch.
        other: String,

        /// Pair name.
        #[arg(long, default_value = "default")]
        name: String,
    },

    /// Show the current branch pair status.
    Status {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,

        /// Show status for every configured pair.
        #[arg(long, conflicts_with = "name")]
        all: bool,

        /// Pair name.
        #[arg(long, default_value = "default")]
        name: String,
    },

    /// Switch to the other branch in a pair.
    Switch {
        /// Show the switch target without changing branches.
        #[arg(long)]
        dry_run: bool,

        /// Pair name.
        #[arg(long, default_value = "default")]
        name: String,
    },

    /// Check whether the current repository is ready for paired-branch work.
    Preflight {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,

        /// Pair name.
        #[arg(long, default_value = "default")]
        name: String,

        /// Agent/session name to check for claim conflicts.
        #[arg(long)]
        agent: Option<String>,

        /// Mark claim conflicts older than this duration as stale, for example 30m, 2h, or 1d.
        #[arg(long, requires = "agent")]
        stale_after: Option<String>,
    },

    /// Assert that the current branch matches expected repository state.
    Assert {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,

        /// Pair name to assert membership in.
        #[arg(long)]
        pair: Option<String>,

        /// Branch name to assert as the current branch.
        #[arg(long)]
        branch: Option<String>,

        /// Pair side to assert as the current branch.
        #[arg(long)]
        side: Option<PairSide>,
    },

    /// Claim the current pair and branch for an agent session.
    Claim {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,

        /// Agent/session name to record in the claim.
        #[arg(long)]
        agent: String,

        /// Pair name to claim.
        #[arg(long, default_value = "default")]
        pair: String,
    },

    /// List active agent session claims.
    Claims {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,

        /// Filter claims by agent/session name.
        #[arg(long)]
        agent: Option<String>,

        /// Filter claims by pair name.
        #[arg(long)]
        pair: Option<String>,

        /// Filter claims by branch name.
        #[arg(long)]
        branch: Option<String>,

        /// Filter claims older than this duration, for example 30m, 2h, or 1d.
        #[arg(long)]
        stale_after: Option<String>,
    },

    /// Prune stale agent session claims. Defaults to dry-run.
    PruneClaims {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,

        /// Filter claims by agent/session name.
        #[arg(long)]
        agent: Option<String>,

        /// Filter claims by pair name.
        #[arg(long)]
        pair: Option<String>,

        /// Filter claims by branch name.
        #[arg(long)]
        branch: Option<String>,

        /// Select claims older than this duration, for example 30m, 2h, or 1d.
        #[arg(long)]
        stale_after: String,

        /// Remove matching stale claims. Without this flag, the command is a dry-run.
        #[arg(long)]
        apply: bool,
    },

    /// Release an agent session claim for the current pair and branch.
    Unclaim {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,

        /// Agent/session name that owns the claim.
        #[arg(long)]
        agent: String,

        /// Pair name for the claim.
        #[arg(long, default_value = "default")]
        pair: String,

        /// Branch name for the claim. Defaults to the current branch.
        #[arg(long)]
        branch: Option<String>,
    },

    /// Show a read-only handoff snapshot for agent continuation.
    Handoff {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,

        /// Pair name to include in the handoff snapshot.
        #[arg(long, default_value = "default")]
        name: String,

        /// Agent/session name to check for claim conflicts.
        #[arg(long)]
        agent: Option<String>,
    },

    /// Remove a branch pair.
    Unpair {
        /// Pair name.
        #[arg(long, default_value = "default")]
        name: String,
    },

    /// Rename a branch pair.
    Rename {
        /// Current pair name.
        old: String,

        /// New pair name.
        new: String,
    },

    /// List all branch pairs in the current repository.
    List {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },

    /// Diagnose Git repository state and Zaphod metadata.
    Doctor {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,

        /// Report claims older than this duration as stale, for example 30m, 2h, or 1d.
        #[arg(long)]
        stale_after: Option<String>,
    },

    /// Generate shell completion scripts.
    Completions {
        /// Shell to generate completions for.
        shell: Shell,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PairSide {
    Left,
    Right,
}

#[cfg(test)]
mod tests {
    use super::{Cli, CliCommand, PairSide};
    use clap::{CommandFactory, Parser};
    use clap_complete::Shell;

    #[test]
    fn parser_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_pair_command_with_default_name() {
        let cli = Cli::parse_from(["zaphod", "pair", "feature/api", "feature/ui"]);

        assert_eq!(
            cli.command,
            CliCommand::Pair {
                left: "feature/api".to_owned(),
                right: "feature/ui".to_owned(),
                name: "default".to_owned(),
            }
        );
    }

    #[test]
    fn parses_init_command_with_default_name() {
        let cli = Cli::parse_from(["zaphod", "init", "feature/ui"]);

        assert_eq!(
            cli.command,
            CliCommand::Init {
                other: "feature/ui".to_owned(),
                name: "default".to_owned(),
            }
        );
    }

    #[test]
    fn parses_status_json_flag() {
        let cli = Cli::parse_from(["zaphod", "status", "--json"]);

        assert_eq!(
            cli.command,
            CliCommand::Status {
                json: true,
                all: false,
                name: "default".to_owned(),
            }
        );
    }

    #[test]
    fn parses_status_all_flag() {
        let cli = Cli::parse_from(["zaphod", "status", "--all"]);

        assert_eq!(
            cli.command,
            CliCommand::Status {
                json: false,
                all: true,
                name: "default".to_owned(),
            }
        );
    }

    #[test]
    fn status_all_conflicts_with_name() {
        let error = Cli::try_parse_from(["zaphod", "status", "--all", "--name", "api"])
            .expect_err("reject conflicting status selectors");

        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn parses_global_json_errors_flag() {
        let cli = Cli::parse_from(["zaphod", "--json-errors", "status"]);

        assert!(cli.json_errors);
        assert_eq!(
            cli.command,
            CliCommand::Status {
                json: false,
                all: false,
                name: "default".to_owned(),
            }
        );
    }

    #[test]
    fn parses_list_json_flag() {
        let cli = Cli::parse_from(["zaphod", "list", "--json"]);

        assert_eq!(cli.command, CliCommand::List { json: true });
    }

    #[test]
    fn parses_rename_command() {
        let cli = Cli::parse_from(["zaphod", "rename", "default", "api"]);

        assert_eq!(
            cli.command,
            CliCommand::Rename {
                old: "default".to_owned(),
                new: "api".to_owned(),
            }
        );
    }

    #[test]
    fn parses_named_switch_command() {
        let cli = Cli::parse_from(["zaphod", "switch", "--name", "review"]);

        assert_eq!(
            cli.command,
            CliCommand::Switch {
                dry_run: false,
                name: "review".to_owned(),
            }
        );
    }

    #[test]
    fn parses_switch_dry_run_flag() {
        let cli = Cli::parse_from(["zaphod", "switch", "--dry-run"]);

        assert_eq!(
            cli.command,
            CliCommand::Switch {
                dry_run: true,
                name: "default".to_owned(),
            }
        );
    }

    #[test]
    fn parses_preflight_command() {
        let cli = Cli::parse_from([
            "zaphod",
            "preflight",
            "--json",
            "--name",
            "api",
            "--agent",
            "codex",
            "--stale-after",
            "2h",
        ]);

        assert_eq!(
            cli.command,
            CliCommand::Preflight {
                json: true,
                name: "api".to_owned(),
                agent: Some("codex".to_owned()),
                stale_after: Some("2h".to_owned()),
            }
        );
    }

    #[test]
    fn preflight_stale_after_requires_agent() {
        let error = Cli::try_parse_from(["zaphod", "preflight", "--stale-after", "2h"])
            .expect_err("reject stale claim window without agent");

        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
    }

    #[test]
    fn parses_assert_command() {
        let cli = Cli::parse_from([
            "zaphod",
            "assert",
            "--json",
            "--pair",
            "api",
            "--branch",
            "feature/api",
            "--side",
            "left",
        ]);

        assert_eq!(
            cli.command,
            CliCommand::Assert {
                json: true,
                pair: Some("api".to_owned()),
                branch: Some("feature/api".to_owned()),
                side: Some(PairSide::Left),
            }
        );
    }

    #[test]
    fn parses_claim_command() {
        let cli = Cli::parse_from([
            "zaphod", "claim", "--json", "--agent", "codex", "--pair", "api",
        ]);

        assert_eq!(
            cli.command,
            CliCommand::Claim {
                json: true,
                agent: "codex".to_owned(),
                pair: "api".to_owned(),
            }
        );
    }

    #[test]
    fn parses_claims_command() {
        let cli = Cli::parse_from([
            "zaphod",
            "claims",
            "--json",
            "--agent",
            "codex",
            "--pair",
            "api",
            "--branch",
            "feature/api",
            "--stale-after",
            "2h",
        ]);

        assert_eq!(
            cli.command,
            CliCommand::Claims {
                json: true,
                agent: Some("codex".to_owned()),
                pair: Some("api".to_owned()),
                branch: Some("feature/api".to_owned()),
                stale_after: Some("2h".to_owned()),
            }
        );
    }

    #[test]
    fn parses_prune_claims_command() {
        let cli = Cli::parse_from([
            "zaphod",
            "prune-claims",
            "--json",
            "--agent",
            "codex",
            "--pair",
            "api",
            "--branch",
            "feature/api",
            "--stale-after",
            "2h",
            "--apply",
        ]);

        assert_eq!(
            cli.command,
            CliCommand::PruneClaims {
                json: true,
                agent: Some("codex".to_owned()),
                pair: Some("api".to_owned()),
                branch: Some("feature/api".to_owned()),
                stale_after: "2h".to_owned(),
                apply: true,
            }
        );
    }

    #[test]
    fn parses_unclaim_command() {
        let cli = Cli::parse_from([
            "zaphod",
            "unclaim",
            "--agent",
            "codex",
            "--branch",
            "feature/api",
        ]);

        assert_eq!(
            cli.command,
            CliCommand::Unclaim {
                json: false,
                agent: "codex".to_owned(),
                pair: "default".to_owned(),
                branch: Some("feature/api".to_owned()),
            }
        );
    }

    #[test]
    fn parses_handoff_command() {
        let cli = Cli::parse_from([
            "zaphod", "handoff", "--json", "--name", "api", "--agent", "codex",
        ]);

        assert_eq!(
            cli.command,
            CliCommand::Handoff {
                json: true,
                name: "api".to_owned(),
                agent: Some("codex".to_owned()),
            }
        );
    }

    #[test]
    fn parses_completions_command() {
        let cli = Cli::parse_from(["zaphod", "completions", "bash"]);

        assert_eq!(cli.command, CliCommand::Completions { shell: Shell::Bash });
    }

    #[test]
    fn parses_doctor_command() {
        let cli = Cli::parse_from(["zaphod", "doctor"]);

        assert_eq!(
            cli.command,
            CliCommand::Doctor {
                json: false,
                stale_after: None,
            }
        );
    }

    #[test]
    fn parses_doctor_json_flag() {
        let cli = Cli::parse_from(["zaphod", "doctor", "--json", "--stale-after", "2h"]);

        assert_eq!(
            cli.command,
            CliCommand::Doctor {
                json: true,
                stale_after: Some("2h".to_owned()),
            }
        );
    }
}
