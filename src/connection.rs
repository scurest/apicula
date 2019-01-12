//! Model connection to textures, animations, etc.
//!
//! There is no data in the actual Nitro files (that I know of) that tells us
//! which animation applies to which model or what texture to use when a model
//! says to use texture with name such-and-such. Presumably game code would just
//! call supply the right files to the right calls. That leaves us to figure it
//! out for ourselves. This modules contains the heuristics for that.

use clap::ArgMatches;
use db::{Database, AnimationId, TextureId, PaletteId, ModelId};

/// A Connection records interrelationships between Nitro resources, namely how
/// all the other resources relate to the models.
pub struct Connection {
    pub models: Vec<ModelConnection>,
}

/// Records for a model (1) what texture/palette each material should use, and
/// (2) which animations can be applied to it.
pub struct ModelConnection {
    pub materials: Vec<MaterialConnection>,
    /// List of animations that can be applied to the model.
    pub animations: Vec<AnimationId>,
}

/// Result of resolving which texture/palette a material should use.
pub enum MaterialConnection {
    NoTexture,
    TextureMissing,
    TextureOkNoPalette {
        texture: Match<TextureId>,
    },
    TextureOkPaletteMissing {
        texture: Match<TextureId>,
     },
    TextureOkPaletteOk {
        texture: Match<TextureId>,
        palette: Match<PaletteId>,
    },
}

/// Result of resolving a texture/palette name to a matching texture/palette ID.
#[derive(Copy, Clone)]
pub struct Match<T: Copy> {
    pub id: T,
    /// True if the match was the best possible amoung all our candidates
    /// (high confidence we picked the correct one).
    pub best: bool,
}

impl MaterialConnection {
    pub fn texture(&self) -> Option<Match<TextureId>> {
        match *self {
            MaterialConnection::NoTexture |
            MaterialConnection::TextureMissing =>
                None,
            MaterialConnection::TextureOkNoPalette { texture } |
            MaterialConnection::TextureOkPaletteMissing { texture } |
            MaterialConnection::TextureOkPaletteOk { texture, .. } =>
                Some(texture)
        }
    }
}

#[derive(Copy, Clone)]
pub struct ConnectionOptions {
    /// Let's you disable the animation heuristic, which can be inaccurate, and
    /// apply all animations.
    pub animation_heuristic_on: bool,
}

impl ConnectionOptions {
    /// Creates a ConnectionOptions from the CLI arguments.
    pub fn from_arg_matches(matches: &ArgMatches) -> ConnectionOptions {
        let animation_heuristic_on = !matches.is_present("apply_any_animation");
        ConnectionOptions { animation_heuristic_on }
    }
}

impl Connection {
    pub fn build(db: &Database, options: ConnectionOptions) -> Connection {
        let models = db.models.iter().enumerate().map(|(model_id, model)| {
            let materials = (0..model.materials.len())
                .map(|material_id| {
                    resolve_material(db, model_id, material_id)
                }).collect();
            let animations = find_applicable_animations(db, model_id, options);

            ModelConnection { materials, animations }
        }).collect();
        Connection { models }
    }
}


// HEURISTICS:

/// TO RESOLVE A MATERIAL TEXTURE: The model file stores the texture name, so
/// our initial set of candidates is all the texture in the DB with that name.
/// If the material specifies a palette, we won't match a texture that doesn't
/// require a palette, and the other way around too. If there are multiple
/// candidates, we prefer one from the same file as the model (this is a good
/// heuristic for models that store their textures/palettes in the same NSBMD
/// file, which most do, but it doesn't help the textures are in a separate
/// NSBTX file). If there are still multiple candidates, we prefer the first one
/// (but we record a note about the match being tentative).
///
/// Palettes are subsequently resolved similarly, prefering palettes from the
/// same file as the texture.
fn resolve_material(db: &Database, model_id: ModelId, material_idx: usize) -> MaterialConnection {
    let material = &db.models[model_id].materials[material_idx];

    let texture_name = match material.texture_name {
        None => return MaterialConnection::NoTexture,
        Some(ref name) => name,
    };
    let has_palette = material.palette_name.is_some();

    // Resolve the texture name. Start with all textures with the right name.
    let mut candidates = db.textures_by_name[texture_name].clone();

    // If the material specifies a palette, discard candidates that don't use
    // one, and conversely.
    candidates.retain(|&tex_id| {
        let requires_palette = db.textures[tex_id].params
            .format().desc().requires_palette;
        requires_palette == has_palette
    });

    // If there are candidates in the same file as the model we prefer them;
    // discard the others.
    let is_in_model_file = |&tex_id: &TextureId| {
        db.textures_found_in[tex_id] == db.models_found_in[model_id]
    };
    if candidates.iter().any(is_in_model_file) {
        candidates.retain(is_in_model_file)
    }

    let texture_match = match candidates.len() {
        0 => return MaterialConnection::TextureMissing,
        n => Match { id: candidates[0], best: n == 1 },
    };

    // If there was no palette, we're done!
    if !has_palette {
        return MaterialConnection::TextureOkNoPalette {
            texture: texture_match,
        };
    }

    // Otherwise, resolve the palette. Start with candidates that have the right
    // name.
    let palette_name = material.palette_name.as_ref().unwrap();
    let mut candidates = db.palettes_by_name[palette_name].clone();

    // If there are candidates in the same file as the texture we prefer them;
    // discard the others.
    let texture_file = db.textures_found_in[texture_match.id];
    let is_in_tex_file = |&pal_id: &PaletteId| {
        db.palettes_found_in[pal_id] == texture_file
    };
    if candidates.iter().any(is_in_tex_file) {
        candidates.retain(is_in_tex_file)
    }

    let palette_match = match candidates.len() {
        0 => return MaterialConnection::TextureOkPaletteMissing { texture: texture_match },
        n => Match { id: candidates[0], best: n == 1 },
    };

    MaterialConnection::TextureOkPaletteOk {
        texture: texture_match,
        palette: palette_match,
    }
}

/// TO DETERMINE WHICH ANIMATIONS APPLY: An animation varies the values of the
/// model's object matrices, so the obvious heuristic is that an animation
/// applies to a model if it animates as many objects as the model has. This
/// obviously gives false-positives since any two models with the same number of
/// objects have the same set of animations applied to them. Surprisingly it
/// also gives false-negatives: some animations that certainly go with a certain
/// model have a different number of objects (maybe so it can be re-used amoung
/// multiple models??).
///
/// To solve the second issue the user is given the option of disabling this
/// heuristic and applying applying all the animations to every model. This,
/// together with the first issue, is the main impediment to batch-converting
/// whole games.
fn find_applicable_animations(db: &Database, model_id: ModelId, options: ConnectionOptions)
-> Vec<AnimationId> {
    if !options.animation_heuristic_on {
        // All animations apply. Let's try not to worry about how big this is :o
        return (0..db.animations.len()).collect();
    }

    // Only animations with the same number of objects apply.
    let num_model_objs = db.models[model_id].objects.len();
    (0..db.animations.len())
        .filter(|&anim_id| {
            let num_anim_objs = db.animations[anim_id].objects_curves.len();
            num_anim_objs == num_model_objs
        })
        .collect()
}
