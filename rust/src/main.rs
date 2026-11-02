//! The `chartedcode` executable.

use std::process::ExitCode;

fn main() -> ExitCode {
    let argv = std::env::args_os()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    ExitCode::from(chartedcode_core::cli::run(argv))
}
