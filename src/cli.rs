use clap::{Parser, Subcommand};
use clap_complete::Shell;

#[derive(Debug, Parser)]
#[command(
    name = "zaphod",
    version,
    about = "A cautious Git workflow CLI for paired branches.",
    long_about = None
)]
pub struct Cli {
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

    /// Show the current branch pair status.
    Status {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,

        /// Pair name.
        #[arg(long, default_value = "default")]
        name: String,
    },

    /// Switch to the other branch in a pair.
    Switch {
        /// Pair name.
        #[arg(long, default_value = "default")]
        name: String,
    },

    /// Remove a branch pair.
    Unpair {
        /// Pair name.
        #[arg(long, default_value = "default")]
        name: String,
    },

    /// List all branch pairs in the current repository.
    List,

    /// Diagnose Git repository state and Zaphod metadata.
    Doctor,

    /// Generate shell completion scripts.
    Completions {
        /// Shell to generate completions for.
        shell: Shell,
    },
}

#[cfg(test)]
mod tests {
    use super::{Cli, CliCommand};
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
    fn parses_status_json_flag() {
        let cli = Cli::parse_from(["zaphod", "status", "--json"]);

        assert_eq!(
            cli.command,
            CliCommand::Status {
                json: true,
                name: "default".to_owned(),
            }
        );
    }

    #[test]
    fn parses_named_switch_command() {
        let cli = Cli::parse_from(["zaphod", "switch", "--name", "review"]);

        assert_eq!(
            cli.command,
            CliCommand::Switch {
                name: "review".to_owned(),
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

        assert_eq!(cli.command, CliCommand::Doctor);
    }
}
