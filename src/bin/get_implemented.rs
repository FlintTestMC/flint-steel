//! Prints all blocks or items with a non-default behavior implemented in `SteelMC`.
use std::process::ExitCode;

use flint_steel::adapter::{
    get_implemented_blocks, get_implemented_entities, get_implemented_items,
};

fn main() -> ExitCode {
    let ids = match std::env::args().nth(1).as_deref() {
        Some("blocks") => {
            flint_steel::init();
            get_implemented_blocks()
        }
        Some("items") => {
            flint_steel::init();
            get_implemented_items()
        }
        Some("entities") => {
            flint_steel::init();
            get_implemented_entities()
        }
        _ => {
            eprintln!("usage: steel-implemented <blocks|items>");
            return ExitCode::FAILURE;
        }
    };
    for id in ids {
        println!("{id}");
    }
    ExitCode::SUCCESS
}
