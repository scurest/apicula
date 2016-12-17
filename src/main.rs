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

#[macro_use]
mod errors;
#[macro_use]
mod util;
mod convert;
mod geometry;
mod index_builder;
mod joint_builder;
mod nitro;
mod viewer;

use errors::Result;
use std::fs::File;
use std::io::Read;
use util::cur::Cur;

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
    let app = clap_app!(demense =>
        (@setting SubcommandRequiredElseHelp)
        (version: "0.1")
        (about: "NSBMD model viewer/converter")
        (@subcommand view =>
            (about: "View a model")
            (alias: "v")
            (@arg INPUT: +required "BMD0 file")
        )
        (@subcommand convert =>
            (about: "Convert a model to COLLADA")
            (alias: "c")
            (@arg INPUT: +required "BMD0 file")
            (@arg OUTPUT: -o --output +required +takes_value "output directory")
        )
    );
    let matches = app.get_matches();
    let subcmd = matches.subcommand_name().unwrap();

    let input_file = matches
        .subcommand_matches(subcmd).unwrap()
        .value_of_os("INPUT").unwrap();
    let mut f = File::open(&input_file).unwrap();
    let mut v: Vec<u8> = vec![];
    f.read_to_end(&mut v).unwrap();
    let cur = Cur::new(&v[..]);
    let bmd = nitro::read_bmd(cur)?;

    match matches.subcommand() {
        ("view", Some(m)) => {
            viewer::main(&bmd)?;
        }
        ("convert", Some(m)) => {
            convert::main(m, &bmd)?;
        }
        _ => {}
    };
    Ok(())
}
