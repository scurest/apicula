mod collada;

use clap::ArgMatches;
use errors::Result;
use nitro::Bmd;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use util::name;

pub fn main(matches: &ArgMatches, bmd: &Bmd) -> Result<()> {
    let model = &bmd.mdl.models[0];

    let out_dir = PathBuf::from(matches.value_of("OUTPUT").unwrap());
    fs::create_dir(&out_dir)?;
    let dae_path = out_dir.join(&format!("{}.dae", name::IdFmt(&model.name)));
    let mut f = File::create(dae_path)?;

    let mut s = String::new();
    collada::write(&mut s, model)?;

    f.write_all(s.as_bytes())?;

    Ok(())
}
