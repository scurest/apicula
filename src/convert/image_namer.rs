use db::{Database, TextureId, PaletteId};
use nitro::Name;
use std::collections::HashMap;
use util::namers::UniqueNamer;

pub struct ImageNamer {
    pub namer: UniqueNamer,
    pub names: HashMap<(TextureId, Option<PaletteId>), String>,
}

impl ImageNamer {
    pub fn build(db: &Database) -> ImageNamer {
        let mut image_namer = ImageNamer {
            namer: UniqueNamer::new(),
            names: HashMap::new(),
        };

        for image_desc in db.material_table.values() {
            use db::ImageDesc;
            let (texture_id, palette_id) = match *image_desc {
                ImageDesc::Image { texture_id, palette_id } =>
                    (texture_id, palette_id),
                _ => continue,
            };

            let namer = &mut image_namer.namer;
            image_namer.names.entry((texture_id, palette_id)).or_insert_with(|| {
                let name = format!("{}", db.textures[texture_id].name.print_safe());
                namer.get_fresh_name(name)
            });
        }

        image_namer
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
            if texture.params.format == 7 {
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
    let mut idx = res.0.iter().rposition(|&x| x != b'0')
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
