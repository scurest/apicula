use clap::ArgMatches;
use errors::Result;
use db::Database;
use connection::{Connection, ConnectionOptions, MaterialConnection, Match};

pub fn main(matches: &ArgMatches) -> Result<()> {
    let db = Database::from_arg_matches(matches)?;

    let conn_options = ConnectionOptions::from_arg_matches(matches);
    let conn = Connection::build(&db, conn_options);

    db.print_status();

    println!();

    for model_id in 0..db.models.len() {
        model_info(&db, &conn, model_id);
    }
    for texture_id in 0..db.textures.len() {
        texture_info(&db, texture_id);
    }
    for palette_id in 0..db.palettes.len() {
        palette_info(&db, palette_id);
    }
    for animation_id in 0..db.animations.len() {
        animation_info(&db, animation_id);
    }
    for pattern_id in 0..db.patterns.len() {
        pattern_info(&db, pattern_id);
    }
    for mat_anim_id in 0..db.mat_anims.len() {
        mat_anim_info(&db, mat_anim_id);
    }

    Ok(())
}


fn model_info(db: &Database, conn: &Connection, model_id: usize) {
    let model = &db.models[model_id];
    println!("Model {}:", model_id);
    println!("  Name: {:?}", model.name);
    println!("  Found In: {}",
        db.file_paths[db.models_found_in[model_id]].to_string_lossy());
    println!("  Num Meshes: {}", model.meshes.len());
    println!("  Objects ({} total):", model.objects.len());
    for (i, object) in model.objects.iter().enumerate() {
        print!("    Object {}: {:?} ", i, object.name);
        println!("({}{}{})",
            object.trans.map(|_| "T").unwrap_or("-"),
            object.rot.map(|_| "R").unwrap_or("-"),
            object.scale.map(|_| "S").unwrap_or("-"),
        );
    }
    println!("  Materials ({} total):", model.materials.len());
    for (i, material) in model.materials.iter().enumerate() {
        println!("    Material {}:", i);
        println!("      Name: {:?}", material.name);
        if let Some(name) = material.texture_name {
            print!("      Texture: {:?} ", name);

            match conn.models[model_id].materials[i].texture() {
                None => print!("(not found)"),
                Some(Match { id, best }) => {
                    print!("(matched texture {}", id);
                    if !best {
                        print!(", but tentatively");
                    }
                    print!(")")
                }
            }
            println!();
        }
        if let Some(name) = material.palette_name {
            print!("      Palette: {:?} ", name);

            match conn.models[model_id].materials[i] {
                MaterialConnection::NoTexture =>
                    print!("(palette but no texture!?)"),
                MaterialConnection::TextureMissing { .. } =>
                    print!("(skipped; texture missing)"),
                MaterialConnection::TextureOkNoPalette { .. } =>
                    unreachable!(),
                MaterialConnection::TextureOkPaletteMissing { .. } =>
                    print!("(not found)"),
                MaterialConnection::TextureOkPaletteOk {
                    palette: Match { id, best }, ..
                } => {
                    print!("(matched palette {}", id);
                    if !best {
                        print!(", but tentatively");
                    }
                    print!(")");
                }
            }
            println!();
        }

        let params = material.params;
        println!("      Dimensions: {}x{}", material.width, material.height);
        println!("      Repeat (s,t): ({}, {})", params.repeat_s(), params.repeat_t());
        println!("      Mirror (s,t): ({}, {})", params.mirror_s(), params.mirror_t());
        println!("      Texcoord Transform Mode: {}", params.texcoord_transform_mode());

        println!("      Diffuse Color: {:?}", material.diffuse);
        println!("      Diffuse is Default Vertex Color: {}", material.diffuse_is_default_vertex_color);
        println!("      Ambient Color: {:?}", material.ambient);
        println!("      Specular Color: {:?}", material.specular);
        println!("      Enable Shininess Table: {}", material.enable_shininess_table);
        println!("      Emission: {:?}", material.emission);
        println!("      Alpha: {:?}", material.alpha);

        println!("      Cull: {}",
            match (material.cull_backface, material.cull_frontface) {
                (true, true) => "All (y tho?)",
                (true, false) => "Backfacing",
                (false, true) => "Frontfacing",
                (false, false) => "None",
            }
        );
    }
    println!();
}


fn texture_info(db: &Database, texture_id: usize) {
    let texture = &db.textures[texture_id];
    println!("Texture {}:", texture_id);
    println!("  Name: {:?}", texture.name);
    println!("  Found In: {}",
        db.file_paths[db.textures_found_in[texture_id]].to_string_lossy());

    let params = texture.params;
    println!("  Offset: {:#x}", params.offset());
    println!("  Dimensions: {}x{}", params.width(), params.height());
    println!("  Format: {} ({})", params.format().0, params.format().desc().name);
    println!("  Color 0 Transparent?: {}", params.is_color0_transparent());
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
    println!("  Num Objects: {}", anim.objects_curves.len());
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

fn pattern_info(db: &Database, pat_id: usize) {
    let pat = &db.patterns[pat_id];
    println!("Pattern Animation {}:", pat_id);
    println!("  Name: {:?}", pat.name);
    println!("  Found In: {}",
        db.file_paths[db.patterns_found_in[pat_id]].to_string_lossy());
    println!("  Frames: {}", pat.num_frames);
    println!("  Tracks ({} total):", pat.material_tracks.len());
    for (i, track) in pat.material_tracks.iter().enumerate() {
        println!("    Track {}: {}", i, track.name);
        for key in &track.keyframes {
            println!("      {}: {:?} / {:?}",
                key.frame,
                pat.texture_names[key.texture_idx as usize],
                pat.palette_names[key.palette_idx as usize],
            );
        }
    }
    println!();
}

fn mat_anim_info(db: &Database, mat_anim_id: usize) {
    let mat_anim = &db.mat_anims[mat_anim_id];
    println!("Material Animation {}:", mat_anim_id);
    println!("  Name: {:?}", mat_anim.name);
    println!("  Found In: {}",
        db.file_paths[db.mat_anims_found_in[mat_anim_id]].to_string_lossy());
    println!("  Frames: {}", mat_anim.num_frames);
    println!("  Tracks ({} total):", mat_anim.tracks.len());
    for (i, track) in mat_anim.tracks.iter().enumerate() {
        println!("    Track {}:", i);
        println!("      Name: {}", track.name);
    }
    println!();
}
