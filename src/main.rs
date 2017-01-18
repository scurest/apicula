#![recursion_limit = "1024"] // for error_chain

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
extern crate png as pnglib;

#[macro_use]
mod errors;
#[macro_use]
mod util;
mod convert;
mod files;
mod geometry;
mod index_builder;
mod joint_builder;
mod png;
mod nitro;
mod viewer;

use errors::Result;

fn main() {
    let ret_code = main2();
    std::process::exit(ret_code);
}

fn main2() -> i32 {
    env_logger::init().unwrap();
    match main3() {
        Ok(()) => 0,
        Err(e) => {
            error!("error: {:#?}", e);
            1
        }
    }
}

fn main3() -> Result<()> {
    let app = clap_app!(apicula =>
        (@setting SubcommandRequiredElseHelp)
        (version: "0.1")
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
            (@arg OUTPUT: -o --output +required +takes_value "output directory")
        )
    );
    let matches = app.get_matches();

    match matches.subcommand() {
        ("view", Some(m)) => viewer::main(m)?,
        ("convert", Some(m)) => convert::main(m)?,
        _ => {}
    };
    Ok(())
}
