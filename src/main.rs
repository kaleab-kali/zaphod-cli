use clap::Parser;
use std::process::ExitCode;
use zaphod_cli::cli::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();

    match zaphod_cli::app::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let exit_code = error.exit_code();
            eprintln!("error: {error}");
            ExitCode::from(exit_code)
        }
    }
}
