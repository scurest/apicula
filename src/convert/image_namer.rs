//! Discovers images in a Connection and assigns them names. We use these for
//! image filenames so that models know what the path to a specific image it
//! uses will be.
use crate::db::{Database, TextureId, PaletteId};
use crate::nitro::Name;
use std::collections::HashMap;
use crate::util::namers::UniqueNamer;
use crate::connection::Connection;

type ImageId = (TextureId, Option<PaletteId>);

pub struct ImageNamer {
    pub namer: UniqueNamer,
    pub names: HashMap<ImageId, String>,
    pub used_texture_ids: Vec<bool>, // TODO: BitVec
}

impl ImageNamer {
    pub fn build(db: &Database, conn: &Connection) -> ImageNamer {
        let mut image_namer = ImageNamer {
            namer: UniqueNamer::new(),
            names: HashMap::new(),
            used_texture_ids: vec![false; db.textures.len()],
        };

        // Discovery images from model materials
        for mdl_conn in &conn.models {
            for mat_conn in &mdl_conn.materials {
                match mat_conn.image_id() {
                    Ok(Some(image_id)) =>
                        image_namer.insert_image_id(db, image_id),
                    _ => continue,
                }
            }
        }

        // Discover images from pattern animations
        for mdl_conn in &conn.models {
            for pat_conn in &mdl_conn.patterns {
                let pat = &db.patterns[pat_conn.pattern_id];
                for track in &pat.material_tracks {
                    for keyframe in &track.keyframes {
                        let tex_idx = keyframe.texture_idx as usize;
                        let texture_id = match pat_conn.texture_ids[tex_idx] {
                            Some(id) => id,
                            None => continue,
                        };
                        let pal_idx = keyframe.palette_idx as usize;
                        let palette_id = match pat_conn.palette_ids[pal_idx] {
                            Some(id) => id,
                            None => continue,
                        };
                        let image_id = (texture_id, Some(palette_id));
                        image_namer.insert_image_id(db, image_id);
                    }
                }
            }
        }

        image_namer
    }

    pub fn insert_image_id(&mut self, db: &Database, image_id: ImageId) {
        let texture_name = db.textures[image_id.0].name;
        let namer = &mut self.namer;
        self.names.entry(image_id).or_insert_with(|| {
            namer.get_fresh_name(texture_name.print_safe().to_string())
        });
        self.used_texture_ids[image_id.0] = true;
    }

    /// Discover even more images by guessing, based on their names, which
    /// palettes go with which textures.
    pub fn add_more_images(&mut self, db: &Database) {
        let mut num_guesses = 0;
        let mut still_unextracted = false;

        for (texture_id, texture) in db.textures.iter().enumerate() {
            if self.used_texture_ids[texture_id] {
                continue;
            }

            // Direct color textures don't need a palette.
            if !texture.params.format().desc().requires_palette {
                self.insert_image_id(db, (texture_id, None));
                num_guesses += 1;
                continue;
            }

            // If there's one palette, guess it
            if db.palettes.len() == 1 {
                self.insert_image_id(db, (texture_id, Some(0)));
                num_guesses += 1;
                continue;
            }

            // Guess palette name == texture name
            if let Some(ids) = db.palettes_by_name.get(&texture.name) {
                self.insert_image_id(db, (texture_id, Some(ids[0])));
                num_guesses += 1;
                continue;
            }

            // Guess palette name == texture_name + "_pl"
            if let Some(ids) = db.palettes_by_name.get(&append_pl(&texture.name)) {
                self.insert_image_id(db, (texture_id, Some(ids[0])));
                num_guesses += 1;
                continue;
            }

            still_unextracted = true;
        }

        info!("Guessed {} new images (for --more-images)", num_guesses);
        if still_unextracted {
            info!("There are still unextracted textures though");
        }
    }
}

/// Append "_pl" to the end of a name.
fn append_pl(name: &Name) -> Name {
    let mut res = name.clone();

    // Find the index of the first NUL byte in the suffix of NUL bytes.
    let mut idx = res.0.iter().rposition(|&x| x != b'\0')
        .map(|pos| pos + 1)
        .unwrap_or(0);

    // Append as much of b"_pl" as will fit.
    for &b in b"_pl" {
        if idx == res.0.len() { break; }
        res.0[idx] = b;
        idx += 1;
    }

    res
}
