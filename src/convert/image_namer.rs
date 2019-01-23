use db::{Database, TextureId, PaletteId};
use nitro::Name;
use std::collections::HashMap;
use util::namers::UniqueNamer;
use connection::Connection;

type ImageId = (TextureId, Option<PaletteId>);

pub struct ImageNamer {
    pub namer: UniqueNamer,
    pub names: HashMap<ImageId, String>,
}

impl ImageNamer {
    pub fn build(db: &Database, conn: &Connection) -> ImageNamer {
        let mut image_namer = ImageNamer {
            namer: UniqueNamer::new(),
            names: HashMap::new(),
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
    }

    pub fn add_more_images(&mut self, db: &Database) {
        let mut num_guesses = 0;

        for (texture_id, texture) in db.textures.iter().enumerate() {

            let mut guess = |(texture_id, palette_id)| {
                use std::collections::hash_map::Entry;
                let namer = &mut self.namer;
                match self.names.entry((texture_id, palette_id)) {
                    Entry::Vacant(ve) => {
                        let name = format!("{}", db.textures[texture_id].name.print_safe());
                        ve.insert(namer.get_fresh_name(name));
                        num_guesses += 1;
                    }
                    _ => (),
                };
            };

            // Direct color textures don't need a palette.
            if !texture.params.format().desc().requires_palette {
                guess((texture_id, None));
                continue;
            }

            let mut guess_palette_name = |name: &Name| {
                if let Some(ids) = db.palettes_by_name.get(name) {
                    guess((texture_id, Some(ids[0])));
                }
            };

            guess_palette_name(&texture.name);
            guess_palette_name(&append_pl(&texture.name));
        }

        if num_guesses > 0 {
            info!("guessed {} new images (for --more-textures)", num_guesses);
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
