#![recursion_limit = "1024"] // for error_chain

#[macro_use]
extern crate log;
#[macro_use]
extern crate error_chain;
extern crate cgmath;
extern crate env_logger;
#[macro_use]
extern crate glium;
extern crate time;

#[macro_use]
mod errors;
#[macro_use]
mod util;
mod nitro;
mod gfx;
mod viewer;
mod geometry;
mod render;

use std::fs::File;
use std::io::Read;
use util::cur::Cur;

fn main() {
    env_logger::init().unwrap();

    let arg = std::env::args().nth(1).unwrap();
    let mut f = File::open(&arg).unwrap();
    let mut v: Vec<u8> = vec![];
    f.read_to_end(&mut v).unwrap();

    let cur = Cur::new(&v[..]);
    let res = nitro::read_bmd(cur);

    match res {
        Ok(bmd) => {
            let model = &bmd.mdl.models[0];
            viewer::viewer(model, &bmd.tex).unwrap();
        }
        Err(e) => error!("err {:#?}", e),
    }
}
