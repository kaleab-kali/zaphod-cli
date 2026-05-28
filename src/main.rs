use clap::Parser;
use std::process::ExitCode;
use zaphod_cli::cli::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json_errors = cli.json_errors;

    match zaphod_cli::app::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let exit_code = error.exit_code();
            if json_errors {
                eprintln!(
                    "{}",
                    serde_json::json!({
                        "error": {
                            "kind": error.kind(),
                            "message": error.to_string(),
                            "exit_code": exit_code,
                        }
                    })
                );
            } else {
                eprintln!("error: {error}");
            }
            ExitCode::from(exit_code)
        }
    }
}
