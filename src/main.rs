//! apicula, NDS model viewer/converter

#![recursion_limit="128"]

#[macro_use]
extern crate log;
#[macro_use]
extern crate glium;
#[macro_use]
extern crate json;

#[macro_use]
mod errors;
#[macro_use]
mod util;
mod cli;
mod convert;
mod decompress;
mod extract;
mod nds;
mod nitro;
mod viewer;
mod db;
mod info;
mod primitives;
mod skeleton;
mod logger;
mod connection;
mod version;

use errors::Result;

fn main() {
    let ret_code = match main2() {
        Ok(()) => 0,
        Err(e) => {
            error!("{}", e);
            1
        }
    };
    std::process::exit(ret_code);
}

fn main2() -> Result<()> {
    init_logger(0);
    let args = cli::parse_cli_args();
    match args.subcommand {
        "extract" => extract::main(&args)?,
        "view" => viewer::main(&args)?,
        "convert" => convert::main(&args)?,
        "info" => info::main(&args)?,
        _ => unimplemented!(),
    }
    Ok(())
}

pub fn init_logger(verbosity: u64) {
    use log::Level;
    let max_log_level = match verbosity {
        0 => Level::Info,
        1 => Level::Debug,
        _ => Level::Trace,
    };
    logger::init(max_log_level);
}
