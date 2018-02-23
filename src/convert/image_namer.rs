use db::Database;
use nitro::{Name, model::Material};
use std::collections::HashMap;
use util::namers::UniqueNamer;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ImageSpec {
    pub texture_name: Name,
    pub palette_name: Option<Name>,
}

impl ImageSpec {
    pub fn from_material(material: &Material) -> Option<ImageSpec> {
        material.texture_name.map(|name| {
            ImageSpec { texture_name: name, palette_name: material.palette_name }
        })
    }
}

pub struct ImageNamer {
    pub namer: UniqueNamer,
    pub names: HashMap<ImageSpec, String>,
}

impl ImageNamer {
    pub fn build(db: &Database) -> ImageNamer {
        let mut image_namer = ImageNamer {
            namer: UniqueNamer::new(),
            names: HashMap::new(),
        };

        for model in &db.models {
            for material in &model.materials {
                if material.texture_name.is_none() {
                    continue;
                }

                let spec = ImageSpec {
                    texture_name: material.texture_name.unwrap(),
                    palette_name: material.palette_name,
                };

                if spec_in_db(db, &spec) {
                    image_namer.add_spec(spec);
                }
            }
        }

        image_namer
    }

    pub fn add_more_images(&mut self, db: &Database) {
        for texture in &db.textures {
            // Direct color textures don't need a palette.
            if texture.params.format == 7 {
                self.add_spec(ImageSpec {
                    texture_name: texture.name,
                    palette_name: None,
                });
                continue;
            }

            let mut guess_palette_name = |name: &Name| {
                if db.palettes_by_name.contains_key(&name) {
                    self.add_spec(ImageSpec {
                        texture_name: texture.name,
                        palette_name: Some(*name),
                    });
                }
            };
            guess_palette_name(&texture.name);
            guess_palette_name(&append_pl(&texture.name));
        }

    }

    fn add_spec(&mut self, spec: ImageSpec) {
        let namer = &mut self.namer;
        self.names.entry(spec.clone()).or_insert_with(|| {
            namer.get_fresh_name(
                format!("{}", spec.texture_name.print_safe())
            )
        });
    }

}

/// Check if the given texture and palette (if given) names occur in the
/// database.
fn spec_in_db(db: &Database, spec: &ImageSpec) -> bool {
    if !db.textures_by_name.contains_key(&spec.texture_name) {
        return false;
    }
    if let Some(ref palette_name) = spec.palette_name {
        if !db.palettes_by_name.contains_key(&palette_name) {
            return false;
        }
    }
    return true;
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
