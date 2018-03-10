use clap::ArgMatches;
use errors::Result;
use db::Database;

pub fn main(matches: &ArgMatches) -> Result<()> {
    let db = Database::from_arg_matches(matches)?;

    db.print_status();
    println!();
    for model_id in 0..db.models.len() {
        model_info(&db, model_id);
    }
    println!();
    for texture_id in 0..db.textures.len() {
        texture_info(&db, texture_id);
    }
    println!();
    for palette_id in 0..db.palettes.len() {
        palette_info(&db, palette_id);
    }
    println!();
    for animation_id in 0..db.animations.len() {
        animation_info(&db, animation_id);
    }

    Ok(())
}


fn model_info(db: &Database, model_id: usize) {
    let model = &db.models[model_id];
    println!("Model {}:", model_id);
    println!("  Name: {:?}", model.name);
    println!("  Found In: {}",
        db.file_paths[db.models_found_in[model_id]].to_string_lossy());
    println!("  Up Scale: {:?}", model.up_scale);
    println!("  Down Scale: {:?}", model.down_scale);
    println!("  Num Meshes: {}", model.meshes.len());
    println!("  Num Objects: {}", model.objects.len());
    println!("  Num Materials: {}", model.materials.len());
    println!("  Objects:");
    for (i, object) in model.objects.iter().enumerate() {
        println!("    Object {}: {:?}", i, object.name);
    }
    println!("  Materials:");
    for (i, material) in model.materials.iter().enumerate() {
        println!("    Material {}:", i);
        if let Some(name) = material.texture_name {
            println!("      Texture: {:?}", name);
        }
        if let Some(name) = material.palette_name {
            println!("      Palette: {:?}", name);
        }
        use db::ImageDesc;
        match db.material_table[&(model_id, i)] {
            ImageDesc::NoImage => (),
            ImageDesc::Missing => {
                println!("      Texture/Palette Not Found");
            }
            ImageDesc::Image { texture_id, palette_id } => {
                println!("      Using Texture Id: {}", texture_id);
                if let Some(id) = palette_id {
                    println!("      Using Palette Id: {}", id);
                }
            }
        }
    }
    println!();
}


fn texture_info(db: &Database, texture_id: usize) {
    let texture = &db.textures[texture_id];
    println!("Texture {}:", texture_id);
    println!("  Name: {:?}", texture.name);
    println!("  Found In: {}",
        db.file_paths[db.textures_found_in[texture_id]].to_string_lossy());
    println!("  Offset: {:#x}", texture.params.offset);
    println!("  Repeat S: {}", texture.params.repeat_s);
    println!("  Repeat T: {}", texture.params.repeat_t);
    println!("  Mirror S: {}", texture.params.mirror_s);
    println!("  Mirror T: {}", texture.params.mirror_t);
    println!("  Width: {}", texture.params.width);
    println!("  Height: {}", texture.params.height);
    println!("  Format: {} ({})", texture.params.format,
        match texture.params.format {
            1 => "32-color, 8-level alpha",
            2 => "4-color",
            3 => "16-color",
            4 => "256-color",
            5 => "compressed",
            6 => "8-color, 32-level alpha",
            7 => "direct color",
            _ => "??",
        }
    );
    println!("  Color 0 Transparent?: {}", texture.params.is_color0_transparent);
    println!("  Texcoord Transform Mode: {}", texture.params.texcoord_transform_mode);
    println!();
}

fn palette_info(db: &Database, palette_id: usize) {
    let palette = &db.palettes[palette_id];
    println!("Palette {}:", palette_id);
    println!("  Name: {:?}", palette.name);
    println!("  Found In: {}",
        db.file_paths[db.palettes_found_in[palette_id]].to_string_lossy());
    println!("  Offset: {:#x}", palette.off);
    println!();
}

fn animation_info(db: &Database, anim_id: usize) {
    let anim = &db.animations[anim_id];
    println!("Animation {}:", anim_id);
    println!("  Name: {:?}", anim.name);
    println!("  Found In: {}",
        db.file_paths[db.animations_found_in[anim_id]].to_string_lossy());
    println!("  Frames: {}", anim.num_frames);
    println!("  Objects: {}", anim.objects_curves.len());
    println!("  TRS Curves:",);
    for (i, trs_curves) in anim.objects_curves.iter().enumerate() {
        println!("    Object {}:", i);

        use nitro::animation::Curve;
        fn curve_info<T>(name: &'static str, curve: &Curve<T>) {
            match *curve {
                Curve::None => { }
                Curve::Constant(_) => {
                    println!("      {}: Constant", name)
                }
                Curve::Samples { start_frame, end_frame, ref values } => {
                    println!("      {}: {} samples (frames {} to {})",
                        name, values.len(), start_frame, end_frame)
                }
            }
        }

        curve_info("Trans X", &trs_curves.trans[0]);
        curve_info("Trans Y", &trs_curves.trans[1]);
        curve_info("Trans Z", &trs_curves.trans[2]);
        curve_info("Rotation", &trs_curves.rotation);
        curve_info("Scale X", &trs_curves.scale[0]);
        curve_info("Scale Y", &trs_curves.scale[1]);
        curve_info("Scale Z", &trs_curves.scale[2]);
    }
    println!();
}
