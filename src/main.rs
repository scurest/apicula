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

#[macro_use]
mod errors;
#[macro_use]
mod util;
mod collada;
mod geometry;
mod gfx;
mod index_builder;
mod nitro;
mod render;
mod viewer;

use std::fs::File;
use std::io::Read;
use util::cur::Cur;

fn main() {
    env_logger::init().unwrap();

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
        )
    );
    let matches = app.get_matches();
    let subcmd = matches.subcommand_name().unwrap();

    let arg = matches
        .subcommand_matches(subcmd).unwrap()
        .value_of_os("INPUT").unwrap();
    let mut f = File::open(&arg).unwrap();
    let mut v: Vec<u8> = vec![];
    f.read_to_end(&mut v).unwrap();

    let cur = Cur::new(&v[..]);
    let res = nitro::read_bmd(cur);

    match res {
        Ok(bmd) => {
            let model = &bmd.mdl.models[0];
            match matches.subcommand_name() {
                Some("view") => {
                    let res = viewer::viewer(model, &bmd.tex);
                    if let Err(e) = res {
                        error!("err {:#?}", e);
                    }
                }
                Some("convert") => {
                    let mut s = String::new();
                    let res = collada::write(&mut s, model);
                    match res {
                        Ok(()) => println!("{}", s),
                        Err(e) => error!("err {:#?}", e),
                    }
                }
                _ => {}
            }
        }
        Err(e) => error!("err {:#?}", e),
    }
}
