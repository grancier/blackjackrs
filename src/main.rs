#![forbid(unsafe_code)]

use std::{
    io::{self, Write},
    process::ExitCode,
};

use blackjackrs::{cli, shoe::seeded_rng};

fn main() -> ExitCode {
    let mut standard_error = io::stderr().lock();
    let (rng, entropy_source) = match seeded_rng() {
        Ok(seeded) => seeded,
        Err(error) => {
            let _write_result = writeln!(standard_error, "Fatal error: {error}");
            return ExitCode::FAILURE;
        }
    };

    let standard_input = io::stdin();
    let standard_output = io::stdout();
    let mut input = standard_input.lock();
    let mut output = standard_output.lock();
    match cli::run(
        &mut input,
        &mut output,
        &mut standard_error,
        rng,
        entropy_source,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _write_result = writeln!(standard_error, "Fatal error: {error}");
            ExitCode::FAILURE
        }
    }
}
