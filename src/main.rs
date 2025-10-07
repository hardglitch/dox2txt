mod tools;
mod core;
mod main_logic;

use crate::main_logic::main_logic;
use crate::tools::sanitizer;
use anyhow::Result;
use std::path::PathBuf;
use std::str::FromStr;


#[derive(PartialEq)]
enum Format {
    Docx,
    Epub,
}

fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    if args.len() < 2 {
        eprintln!("Usage: {} <path/to/your/folder> [-r]", args[0]);
        std::process::exit(1);
    }

    let dir = PathBuf::from_str(&args[1])?;
    main_logic(&dir, &args)?;
    sanitizer(&dir)?;

    Ok(())
}
