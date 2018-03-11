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
extern crate time;
extern crate petgraph;
extern crate png as png_crate;
extern crate regex;
extern crate termcolor;
extern crate atty;

#[macro_use]
mod errors;
#[macro_use]
mod util;
mod convert;
mod decompress;
mod extract;
mod nds;
mod nitro;
mod png;
mod viewer;
mod db;
mod info;
mod primitives;
mod skeleton;
mod logger;

use errors::Result;

// See build.rs.
pub static VERSION: &'static str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    include_str!(concat!(env!("OUT_DIR"), "/git-commit")),
    " ",
    include_str!(concat!(env!("OUT_DIR"), "/compile-date")),
    ")",
);

fn main() {
    let ret_code = match main2() {
        Ok(()) => 0,
        Err(e) => {
            error!("error: {}", e);
            1
        }
    };
    std::process::exit(ret_code);
}

fn main2() -> Result<()> {
    let app = clap_app!(apicula =>
        (@setting SubcommandRequiredElseHelp)
        (version: VERSION)
        (about: "NSBMD model viewer/converter")
        (@arg VERBOSE: -v --verbose +multiple "Print verbose debug info")
        (@subcommand view =>
            (about: "View models")
            (alias: "v")
            (@arg INPUT: +required +multiple "Nitro files")
            (@arg apply_any_animation: --("apply-any-animation")
                "Disable the heuristic that animation only apply to models with \
                the same number of objects. Apply all animations to every model.")
        )
        (@subcommand convert =>
            (about: "Convert models to COLLADA")
            (alias: "c")
            (@arg INPUT: +required +multiple "Nitro file")
            (@arg OUTPUT: -o --output +required +takes_value "Output directory")
            (@arg apply_any_animation: --("apply-any-animation")
                "Disable the heuristic that animation only apply to models with \
                the same number of objects. Apply all animations to every model.")
            (@arg more_textures: --("more-textures") +hidden
                "Try to extract more textures; only textures that are needed for a \
                model are extracted by default (EXPERIMENTAL)")
        )
        (@subcommand extract =>
            (about: "Extract Nitro files from a ROM or archive")
            (alias: "x")
            (@arg INPUT: +required "Input file")
            (@arg OUTPUT: -o --output +required +takes_value "Output directory")
        )
        (@subcommand info =>
            (about: "Display information about Nitro files")
            (alias: "i")
            (@arg INPUT: +required +multiple "Nitro files")
        )
    );
    let matches = app.get_matches();

    // Set the log level from the number of --verbose flags we got.
    init_logger(matches.occurrences_of("VERBOSE"));

    match matches.subcommand() {
        ("view", Some(m)) => viewer::main(m)?,
        ("convert", Some(m)) => convert::main(m)?,
        ("extract", Some(m)) => extract::main(m)?,
        ("info", Some(m)) => info::main(m)?,
        _ => {}
    };
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
