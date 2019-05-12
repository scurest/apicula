mod collada;
mod gltf;
mod image_namer;

use clap::ArgMatches;
use connection::{Connection, ConnectionOptions};
use convert::image_namer::ImageNamer;
use db::Database;
use errors::Result;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use util::namers::UniqueNamer;
use util::OutDir;

pub fn main(matches: &ArgMatches) -> Result<()> {
    let out_dir_path = PathBuf::from(matches.value_of("OUTPUT").unwrap());
    let mut out_dir = OutDir::make_ready(out_dir_path)?;

    let db = Database::from_arg_matches(matches)?;

    db.print_status();

    let conn_options = ConnectionOptions::from_arg_matches(matches);
    let conn = Connection::build(&db, conn_options);

    let format = matches.value_of("FORMAT").unwrap_or("dae");
    if format != "dae" && format != "glb" && format != "gltf" {
        bail!("format should be either dae or glb or gltf");
    }

    let mut image_namer = ImageNamer::build(&db, &conn);
    if matches.is_present("more_textures") {
        image_namer.add_more_images(&db);
    }

    let mut models_written = 0;
    let mut pngs_written = 0;

    // Gives unique names to each model file to avoid name clashes.
    let mut model_file_namer = UniqueNamer::new();

    // Save each model as a COLLADA file
    for (model_id, model) in db.models.iter().enumerate() {
        let name = model_file_namer.get_fresh_name(format!("{}", model.name.print_safe()));
        let mut f = out_dir.create_file(&format!("{}.{}", name, format))?;

        let res = if format == "dae" {
            let s = collada::write(&db, &conn, &image_namer, model_id);
            f.write_all(s.as_bytes()).and_then(|_| f.flush())
        } else if format == "glb" || format == "gltf" {
            let gltf = gltf::to_gltf(&db, &conn, &image_namer, model_id);
            if format == "glb" {
                gltf.write_glb(&mut f)
            } else {
                let bin_file_name = format!("{}.bin", name);
                let mut bin_f = out_dir.create_file(&bin_file_name)?;
                gltf.write_gltf_bin(&mut f, &mut bin_f, &bin_file_name)
            }
        } else {
            unreachable!()
        };

        match res {
            Ok(()) => {
                models_written += 1;
            }
            Err(e) => error!("failed to write {}: {}", name, e),
        }
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

        let dim = (texture.params.width(), texture.params.height());
        let mut png_file = out_dir.create_file(&format!("{}.png", image_name))?;
        match write_rgba(&mut png_file, &rgba.0[..], dim) {
            Ok(()) => {
                pngs_written += 1;
            }
            Err(e) => error!("failed writing PNG: {}", e),
        }
    }

    // Print results
    let plural = |x| if x != 1 { "s" } else { "" };
    let model_file_name = match format {
        "dae" => "DAE",
        "glb" => "GLB",
        "gltf" => "glTF",
        _ => unreachable!(),
    };
    println!(
        "Wrote {} {}{}, {} PNG{}.",
        models_written,
        model_file_name,
        plural(models_written),
        pngs_written,
        plural(pngs_written)
    );

    Ok(())
}

pub fn write_rgba(f: &mut File, rgba: &[u8], dim: (u32, u32)) -> Result<()> {
    use png::{BitDepth, ColorType, Encoder, HasParameters};
    let mut encoder = Encoder::new(f, dim.0, dim.1);
    encoder.set(ColorType::RGBA).set(BitDepth::Eight);

    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;

    Ok(())
}
