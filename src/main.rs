use clap::Parser;
use std::process::ExitCode;
use zaphod_cli::cli::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();

    match zaphod_cli::app::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
