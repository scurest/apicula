use clap::ArgMatches;
use errors::Result;
use db::Database;
use nitro::{Model, Texture, Animation};

pub fn main(matches: &ArgMatches) -> Result<()> {
    let db = Database::from_arg_matches(matches)?;

    db.print_status();

    println!();

    for (i, model) in db.models.iter().enumerate() {
        model_info(i, model);
    }

    println!();

    for (i, texture) in db.textures.iter().enumerate() {
        texture_info(i, texture);
    }

    println!();

    for (i, animation) in db.animations.iter().enumerate() {
        animation_info(i, animation);
    }

    Ok(())
}


fn model_info(id: usize, model: &Model) {
    println!("Model {}:", id);
    println!("  Name: {:?}", model.name);
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
    }
    println!();
}


fn texture_info(id: usize, texture: &Texture) {
    println!("Texture {}:", id);
    println!("  Name: {:?}", texture.name);
    println!("  Offset: {:#x}", texture.params.offset);
    println!("  Repeat S: {}", texture.params.repeat_s);
    println!("  Repeat T: {}", texture.params.repeat_t);
    println!("  Mirror S: {}", texture.params.mirror_s);
    println!("  Mirror S: {}", texture.params.mirror_t);
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

fn animation_info(id: usize, anim: &Animation) {
    println!("Animation {}:", id);
    println!("  Name: {:?}", anim.name);
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
