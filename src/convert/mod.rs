#[macro_use]
mod xml;
mod collada;
mod image_namer;
mod make_invertible;
mod gltf;
mod glb;

use clap::ArgMatches;
use errors::{Result, ResultExt};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use util::namers::UniqueNamer;
use db::Database;
use convert::image_namer::ImageNamer;
use connection::{Connection, ConnectionOptions};

pub fn main(matches: &ArgMatches) -> Result<()> {
    let out_dir = PathBuf::from(matches.value_of("OUTPUT").unwrap());
    fs::create_dir(&out_dir)
        .chain_err(||
            "output directory could not be created -- maybe it \
            already exists?"
        )?;

    let db = Database::from_arg_matches(matches)?;

    db.print_status();

    let conn_options = ConnectionOptions::from_arg_matches(matches);
    let conn = Connection::build(&db, conn_options);

    let format = matches.value_of("FORMAT").unwrap_or("dae");
    if format != "dae" && format != "glb" {
        bail!("format should be either dae or glb");
    }

    let mut image_namer = ImageNamer::build(&db, &conn);

    let mut models_written = 0;
    let mut pngs_written = 0;

    // Gives unique names to each model file to avoid name clashes.
    let mut model_file_namer = UniqueNamer::new();

    // Save each model as a COLLADA file
    for (model_id, model) in db.models.iter().enumerate() {
        let name = model_file_namer.get_fresh_name(format!("{}", model.name.print_safe()));
        let model_file_path = out_dir.join(&format!("{}.{}", name, format));
        let mut f = File::create(model_file_path)?;

        let res = if format == "dae" {
            let s = collada::write(&db, &conn, &image_namer, model_id);
            f.write_all(s.as_bytes()).and_then(|_| f.flush())
        } else if format == "glb" {
            let glb = gltf::to_glb(&db, &conn, &image_namer, model_id);
            glb.write(&mut f)
        } else {
            unreachable!()
        };

        match res {
            Ok(()) => { models_written += 1; },
            Err(e) => error!("failed to write {}: {}", name, e),
        }
    }

    if matches.is_present("more_textures") {
        image_namer.add_more_images(&db);
    }

    // Save PNGs for all the images
    for ((texture_id, palette_id), image_name) in image_namer.names.drain() {
        let texture = &db.textures[texture_id];
        let palette = palette_id.map(|id| &db.palettes[id]);

        use nds::decode_texture;
        let rgba = match decode_texture(texture, palette) {
            Ok(rgba) => rgba,
            Err(e) => {
                error!("error generating image {}, error: {}", image_name, e);
                continue;
            }
        };

        use png::write_rgba;
        let path = out_dir.join(&format!("{}.png", image_name));
        match write_rgba(&path, &rgba.0[..], texture.params.width(), texture.params.height()) {
            Ok(()) => { pngs_written += 1; }
            Err(e) => error!("failed to write {}: {}", path.to_string_lossy(), e),
        }
    }

    // Print results
    let plural = |x| if x != 1 { "s" } else { "" };
    let model_file_name = match format {
        "dae" => "DAE",
        "glb" => "GLB",
        _ => unreachable!(),
    };
    println!("Wrote {} {}{}, {} PNG{}.",
        models_written, model_file_name, plural(models_written),
        pngs_written, plural(pngs_written));

    Ok(())
}
