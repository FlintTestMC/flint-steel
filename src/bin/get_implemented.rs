use std::process::ExitCode;
use anyhow::{Context, Result, bail};
use super::adapter;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    for blocks in implemented
    Ok(())
}