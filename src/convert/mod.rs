#[macro_use]
mod format;
mod collada;
mod context;

use clap::ArgMatches;
use convert::context::Context;
use errors::Result;
use files::BufferHolder;
use files::FileHolder;
use nitro::tex;
use png;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use util::namers::UniqueNamer;

pub fn main(matches: &ArgMatches) -> Result<()> {
    let input_files = matches
        .values_of_os("INPUT").unwrap();
    let buf_holder = BufferHolder::read_files(input_files)?;
    let file_holder = FileHolder::from_buffers(&buf_holder);

    let out_dir = PathBuf::from(matches.value_of("OUTPUT").unwrap());
    fs::create_dir(&out_dir)?;

    let mut ctx = Context::from_files(&file_holder);

    // Gives unique names to each .dae file, so that eg. two models with
    // the same name don't get written to the same file.
    let mut dae_namer = UniqueNamer::new();

    // Save each model as a COLLADA file
    let mut s = String::new();
    for model in &file_holder.models {
        s.clear();
        collada::write(&mut s, &ctx, model)?;

        let name = dae_namer.get_fresh_name(format!("{}", model.name.print_safe()));
        let dae_path = out_dir.join(&format!("{}.dae", name));
        let mut f = File::create(dae_path)?;
        match f.write_all(s.as_bytes()) {
            Ok(()) => {},
            Err(e) => error!("failed to write {}: {}", name, e),
        }
    }

    if matches.is_present("more_textures") {
        ctx.add_more_textures();
    }

    // Save PNGs for all the images referenced in the COLLADA files
    for (image_id, image_name) in &ctx.image_names {
        let tex = &file_holder.texs[image_id.0];
        let texinfo = &tex.texinfo[image_id.1];
        let palinfo = image_id.2.map(|pal_id| &tex.palinfo[pal_id]);

        let res = tex::image::gen_image(tex, texinfo, palinfo);
        let rgba = match res {
            Ok(rgba) => rgba,
            Err(e) => {
                warn!("error generating image {}, error: {:?}", image_name, e);
                continue;
            }
        };

        let path = out_dir.join(&format!("{}.png", image_name));
        png::write_rgba(&path, &rgba[..], texinfo.params.width(), texinfo.params.height())?;
    }

    Ok(())
}
