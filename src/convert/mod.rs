mod collada;
mod image_namer;
mod gltf;

use cli::Args;
use errors::Result;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use util::namers::UniqueNamer;
use util::OutDir;
use db::Database;
use convert::image_namer::ImageNamer;
use connection::{Connection, ConnectionOptions};

pub fn main(args: &Args) -> Result<()> {
    let out_dir_path = PathBuf::from(args.get_opt("output").unwrap());
    let mut out_dir = OutDir::new(out_dir_path)?;

    let db = Database::from_cli_args(args)?;

    db.print_status();

    let conn_options = ConnectionOptions::from_cli_args(args);
    let conn = Connection::build(&db, conn_options);

    let format = args.get_opt("format").map(|s| s.to_str().unwrap())
        .unwrap_or("dae");

    let mut image_namer = ImageNamer::build(&db, &conn);
    if args.flags.contains(&"more-textures") {
        image_namer.add_more_images(&db);
    }

    let mut models_written = 0;
    let mut pngs_written = 0;

    // Gives unique names to each model file to avoid name clashes.
    let mut model_file_namer = UniqueNamer::new();

    for (model_id, model) in db.models.iter().enumerate() {
        debug!("Converting model {} ({})...", model.name, model_id);

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
            Ok(()) => { models_written += 1; },
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
            Ok(()) => { pngs_written += 1; }
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
    println!("Wrote {} {}{}, {} PNG{}.",
        models_written, model_file_name, plural(models_written),
        pngs_written, plural(pngs_written));

    Ok(())
}

pub fn write_rgba(f: &mut File, rgba: &[u8], dim: (u32, u32)) -> Result<()> {
    use png::{Encoder, ColorType, BitDepth};

    let mut encoder = Encoder::new(f, dim.0, dim.1);
    encoder.set_color(ColorType::RGBA);
    encoder.set_depth(BitDepth::Eight);

    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;

    Ok(())
}
