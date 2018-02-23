//! apicula, NDS model viewer/converter

#[macro_use]
extern crate log;
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate glium;
#[macro_use]
extern crate clap;
extern crate cgmath;
extern crate env_logger;
extern crate time;
extern crate petgraph;
extern crate png as png_crate;
extern crate regex;

#[macro_use]
mod errors;
#[macro_use]
mod util;
mod convert;
mod decompress;
mod extract;
mod geometry;
mod nds;
mod nitro;
mod png;
mod viewer;
mod db;

use errors::Result;
use std::env;

pub static VERSION: &'static str = concat!(
    env!("CARGO_PKG_VERSION"), " (", include_str!(concat!(env!("OUT_DIR"), "/git-commit")), ")",
);

fn main() {
    std::process::exit(main2());
}

fn main2() -> i32 {
    init_logger();
    match main3() {
        Ok(()) => 0,
        Err(e) => {
            error!("error: {:#?}", e);
            1
        }
    }
}

fn init_logger() {
    use log;
    use env_logger;

    let mut builder = env_logger::Builder::new();

    // Show warnings by default
    builder.filter(None, log::LevelFilter::Warn);

    if env::var("RUST_LOG").is_ok() {
       builder.parse(&env::var("RUST_LOG").unwrap());
    }

    builder.init();
}

fn main3() -> Result<()> {
    let app = clap_app!(apicula =>
        (@setting SubcommandRequiredElseHelp)
        (version: VERSION)
        (about: "NSBMD model viewer/converter")
        (@subcommand view =>
            (about: "View a model")
            (alias: "v")
            (@arg INPUT: +required +multiple "BMD0 file")
        )
        (@subcommand convert =>
            (about: "Convert a model to COLLADA")
            (alias: "c")
            (@arg INPUT: +required +multiple "BMD0 file")
            (@arg OUTPUT: -o --output +required +takes_value "Output directory")
            (@arg more_textures: --("more-textures") +hidden
                "Try to extract more textures; only textures that are used are \
                extracted by default")
        )
        (@subcommand extract =>
            (about: "Extract Nitro files from a ROM or archive")
            (alias: "x")
            (@arg INPUT: +required "Input file")
            (@arg OUTPUT: -o --output +required +takes_value "Output directory")
        )
    );
    let matches = app.get_matches();

    match matches.subcommand() {
        ("view", Some(m)) => viewer::main(m)?,
        ("convert", Some(m)) => convert::main(m)?,
        ("extract", Some(m)) => extract::main(m)?,
        _ => {}
    };
    Ok(())
}
