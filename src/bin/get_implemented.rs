/// small util
use std::process::ExitCode;
use anyhow::{Context, Result, bail};
use flint_steel::adapter::get_implemented_blocks;

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
    for block in get_implemented_blocks()
    {
        println!("{}", block);
    }
    Ok(())
}