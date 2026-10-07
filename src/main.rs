use nesterm_rust::{
    Result,
    cli::{HELP, Options},
    terminal,
};
use std::io::{self, Write};

fn main() -> std::process::ExitCode {
    match execute() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("nesterm: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn execute() -> Result<()> {
    let options = Options::parse(std::env::args_os().skip(1))?;
    if options.help {
        io::stdout().write_all(HELP.as_bytes())?;
    } else if options.version {
        println!("nesterm {}", env!("CARGO_PKG_VERSION"));
    } else {
        terminal::run(&options)?;
    }
    Ok(())
}
