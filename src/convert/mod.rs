#[macro_use]
mod format;
mod collada;

use clap::ArgMatches;
use errors::Result;
use files::BufferHolder;
use files::FileHolder;
use nitro::name::IdFmt;
use nitro::tex;
use nitro::tex::texpal::find_tex;
use png;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use util::namers::UniqueNamer;
use util::namers::UniqueNameSet;

pub fn main(matches: &ArgMatches) -> Result<()> {
    let input_files = matches
        .values_of_os("INPUT").unwrap();
    let buf_holder = BufferHolder::read_files(input_files)?;
    let file_holder = FileHolder::from_buffers(&buf_holder);

    let out_dir = PathBuf::from(matches.value_of("OUTPUT").unwrap());
    fs::create_dir(&out_dir)?;

    // Gives unique names to each .dae file, so that eg. two models with
    // the same name don't get written to the same file.
    let mut dae_namer = UniqueNamer::new();

    // A `UniqueNameSet` to assign names to all the texture/palette pairs
    // we encounter. Also used to know what PNGs to write afterwards.
    let mut image_name_set = UniqueNameSet::new();

    // Save each model as a COLLADA file
    let mut s = String::new();
    for model in &file_holder.models {
        s.clear();
        collada::write(&mut s, model, &file_holder.animations[..], &mut image_name_set)?;

        let name = dae_namer.get_fresh_name(format!("{}", IdFmt(&model.name)));
        let dae_path = out_dir.join(&format!("{}.dae", name));
        let mut f = File::create(dae_path)?;
        f.write_all(s.as_bytes())?;
    }

    // Save PNGs for all the images referenced in the COLLADA files
    for (&pair, image_name) in image_name_set.names_map().iter() {
        let (tex, texinfo, palinfo) = match find_tex(&file_holder.texs[..], pair) {
            Some(t) => t,
            None => {
                warn!("couldn't find a texture named {:?} for the image {}", pair.0, image_name);
                continue;
            }
        };

        let res = tex::image::gen_image(tex, texinfo, palinfo);
        let rgba = match res {
            Ok(rgba) => rgba,
            Err(e) => {
                warn!("error generating image {}, error: {:#?}", image_name, e);
                continue;
            }
        };

        let path = out_dir.join(&format!("{}.png", image_name));
        png::write_rgba(&path, &rgba[..], texinfo.params.width(), texinfo.params.height())?;
    }

    Ok(())
}
