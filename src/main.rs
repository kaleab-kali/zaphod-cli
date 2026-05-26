use clap::Parser;
use zaphod_cli::cli::{Cli, CliCommand};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        CliCommand::Pair { .. } => not_implemented("pair"),
        CliCommand::Status { .. } => not_implemented("status"),
        CliCommand::Switch { .. } => not_implemented("switch"),
        CliCommand::Unpair { .. } => not_implemented("unpair"),
        CliCommand::List => not_implemented("list"),
    }
}

fn not_implemented(command: &str) {
    eprintln!("zaphod {command}: command is planned but not implemented yet");
    std::process::exit(2);
}
