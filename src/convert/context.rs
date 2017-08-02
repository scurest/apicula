use files::FileHolder;
use nitro::name::Name;
use nitro::tex::texpal::TexPalPair;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::collections::HashSet;
use util::namers::UniqueNamer;

/// Indexes a texture in a FileHolder, namely `fh.texs[id.0].texinfo[id.1]`.
type TextureId = (usize, usize);
/// A `TextureId` together with an optional palette (the palette index is into
/// the same Tex as the texture).
type ImageId = (usize, usize, Option<usize>);

/// Some side-tables containing data about the files in a `FileHolder`.
pub struct Context<'a, 'b: 'a> {
    pub fh: &'a FileHolder<'b>,
    /// Map for looking up textures by name (if multiple have the same name,
    /// only the first is inserted).
    texture_name_table: HashMap<Name, TextureId>,
    // Assigns names to the images we need to generate. The names are needed
    // to write the filename of the image into the COLLADA file.
    pub image_names: HashMap<ImageId, String>,
    /// For picking unique names for the images.
    image_namer: UniqueNamer,
    /// A set of all the `TextureId`s that have images in `image_names`.
    /// (A single texture might have multiple, if it is used with
    /// multiple palettes.)
    textures_with_images: HashSet<TextureId>,
}

impl<'a, 'b> Context<'a, 'b> {
    pub fn from_files(fh: &'a FileHolder<'b>) -> Context<'a,'b> {
        let mut texture_name_table = HashMap::new();
        for (tex_id, tex) in fh.texs.iter().enumerate() {
            for (info_id, texture_info) in tex.texinfo.iter().enumerate() {
                let texture_id = (tex_id, info_id);
                let name = texture_info.name;
                let entry = texture_name_table.entry(name);
                match entry {
                    Entry::Vacant(ve) => {
                        ve.insert(texture_id);
                    }
                    Entry::Occupied(oe) => {
                        warn!("texture {:?} has the same name as {:?}; the former \
                            will never be used. To avoid this conflict, try calling \
                            apicula with as few arguments as possible.",
                            texture_id,
                            oe.get(),
                        );
                    }
                }
            }
        }

        let mut ctx = Context {
            fh,
            texture_name_table,
            image_names: HashMap::new(),
            image_namer: UniqueNamer::new(),
            textures_with_images: HashSet::new(),
        };
        ctx.add_images_from_model_materials();
        ctx
    }

    /// Add all the images needed for model materials to the list of
    /// images.
    pub fn add_images_from_model_materials(&mut self) {
        for model in &self.fh.models {
            for material in &model.materials {
                let texture_name =
                    match material.texture_name {
                        Some(name) => name,
                        None => continue,
                    };
                let texpal = TexPalPair(texture_name, material.palette_name);
                if let Some(image_id) = self.image_id_from_texpal_pair(texpal) {
                    self.add_image_from_id(image_id);
                }
            }
        }
    }

    /// Given an ImageId, pick a name for it and insert it into the list
    /// of images. The name is picked from the name of the `TextureInfo`
    /// the `ImageId` denotes. If the image has already been added, does
    /// nothing.
    pub fn add_image_from_id(&mut self, image_id: ImageId) {
        let entry = self.image_names.entry(image_id);
        match entry {
            Entry::Occupied(_) => {}
            Entry::Vacant(ve) => {
                let texinfo = &self.fh.texs[image_id.0].texinfo[image_id.1];
                let name = self.image_namer.get_fresh_name(
                    format!("{}", texinfo.name.print_safe())
                );
                ve.insert(name);
                self.textures_with_images.insert((image_id.0, image_id.1));
            }
        }
    }

    /// Try to add images for every texture. If the texture requires a palette,
    /// a heuristic is used to guess the corresponding palette.
    pub fn add_more_textures(&mut self) {
        for (tex_id, tex) in self.fh.texs.iter().enumerate() {
            for (texinfo_id, texinfo) in tex.texinfo.iter().enumerate() {
                // Skip textures if we've already generated at least one image
                // for them.
                if self.textures_with_images.contains(&(tex_id, texinfo_id)) {
                    continue;
                }

                if texinfo.params.is_direct_color() {
                    let image_id = (tex_id, texinfo_id, None);
                    self.add_image_from_id(image_id);
                } else {
                    // The texture requires a palette. We'll have to try to guess one.

                    let mut try_palette_name = |palname| {
                        let pal_id = tex.palinfo.iter().position(|pal| pal.name == palname);
                        if pal_id.is_some() {
                            let image_id = (tex_id, texinfo_id, pal_id);
                            self.add_image_from_id(image_id);
                        }
                        pal_id.is_some()
                    };

                    // Guess that the palette name is either (1) the same as the texture name,
                    // or (2) the same but with "_pl" appended.
                    let name = texinfo.name.clone();
                    if !try_palette_name(name) {
                        try_palette_name(append_pl(&name));
                    }
                }
            }
        }
    }

    /// Given a texture name and (optionally) a palette name, find an `ImageId`
    /// that identifies texture and palette infos with those names. The palette
    /// must come from the same Tex as the texture (FIXME maybe?).
    ///
    /// Returns `None` if the texture isn't found.
    pub fn image_id_from_texpal_pair(&self, TexPalPair(texture_name, palette_name): TexPalPair)
    -> Option<ImageId> {
        let texture_id =
            match self.texture_name_table.get(&texture_name) {
                Some(texture_id) => texture_id,
                None => return None,
            };
        let tex_id = texture_id.0;

        let tex = &self.fh.texs[tex_id];
        let palette_pos = palette_name.and_then(|palname| {
            tex.palinfo.iter().position(|pal| pal.name == palname)
        });

        Some((texture_id.0, texture_id.1, palette_pos))
    }

    /// Look-up the image name from texture and palette names.
    pub fn image_name_from_texpal_pair(&self, texpal: TexPalPair) -> Option<&String> {
        let res = self.image_id_from_texpal_pair(texpal);
        res.and_then(|id| self.image_names.get(&id))
    }
}

/// Append "_pl" to the end of a name (ie. in the suffix of NUL bytes).
fn append_pl(texture_name: &Name) -> Name {
    let mut res = texture_name.clone();

    // The name is terminated by a string of NUL bytes.
    // Find the position of the first one.
    let mut end = 16;
    while end != 0 {
        if res.0[end-1] != 0 { break; }
        end -= 1;
    }

    // Append as much of "_pl" to the end of name as will fit.
    let suf = b"_pl";
    for (&b, i) in suf.iter().zip(end .. 16) {
        res.0[i] = b;
    }

    res
}
