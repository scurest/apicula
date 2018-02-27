#[macro_use]
mod format;
mod collada;
mod image_namer;
mod make_invertible;

use clap::ArgMatches;
use errors::{Result, ResultExt};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use util::namers::UniqueNamer;
use db::Database;
use convert::image_namer::ImageNamer;

pub fn main(matches: &ArgMatches) -> Result<()> {
    let out_dir = PathBuf::from(matches.value_of("OUTPUT").unwrap());
    fs::create_dir(&out_dir)
        .chain_err(||
            "output directory could not be created -- maybe it \
            already exists?"
        )?;

    let db = Database::from_arg_matches(matches)?;

    db.print_status();

    let mut image_namer = ImageNamer::build(&db);

    let mut daes_written = 0;
    let mut pngs_written = 0;

    // Gives unique names to each .dae file, so that eg. two models with
    // the same name don't get written to the same file.
    let mut dae_namer = UniqueNamer::new();

    // Save each model as a COLLADA file
    let mut s = String::new();
    for model in &db.models {
        s.clear();

        if collada::write(&mut s, &db, &image_namer, model).is_err() {
            continue;
        }

        let name = dae_namer.get_fresh_name(format!("{}", model.name.print_safe()));
        let dae_path = out_dir.join(&format!("{}.dae", name));
        let mut f = File::create(dae_path)?;
        match f.write_all(s.as_bytes()).and_then(|_| f.flush()) {
            Ok(()) => { daes_written += 1; },
            Err(e) => error!("failed to write {}: {}", name, e),
        }
    }

    if matches.is_present("more_textures") {
        image_namer.add_more_images(&db);
    }

    // Save PNGs for all the images
    for (spec, image_name) in &image_namer.names {
        let texture = &db.textures[db.textures_by_name[&spec.texture_name]];
        let palette = spec.palette_name.map(|name| {
            &db.palettes[db.palettes_by_name[&name]]
        });

        use nitro::decode_image::decode;
        let rgba = match decode(texture, palette) {
            Ok(rgba) => rgba,
            Err(e) => {
                error!("error generating image {}, error: {}", image_name, e);
                continue;
            }
        };

        use png::write_rgba;
        let path = out_dir.join(&format!("{}.png", image_name));
        match write_rgba(&path, &rgba[..], texture.params.width, texture.params.height) {
            Ok(()) => { pngs_written += 1; }
            Err(e) => error!("failed to write {}: {}", path.to_string_lossy(), e),
        }
    }

    // Print results
    let plural = |x| if x != 1 { "s" } else { "" };
    println!("Wrote {} DAE{}, {} PNG{}.",
        daes_written, plural(daes_written), pngs_written, plural(pngs_written));

    Ok(())
}
