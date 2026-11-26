use std::process::ExitCode;

use clap::Parser;
use pdf_toolkit::cli::{Cli, Command};
use pdf_toolkit::compress::run_compress;
use pdf_toolkit::merge::run_merge;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Compress(args) => match run_compress(args) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("error: {err}");
                1
            }
        },
        Command::Merge(args) => match run_merge(args) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("error: {err}");
                1
            }
        },
    };
    ExitCode::from(code)
}
