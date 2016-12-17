use nitro::mdl::Model;
use std::collections::HashMap;
use std::collections::HashSet;
use util::name;
use util::name::Name;

pub type ImageNames = HashMap<TexturePalettePair, String>;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct TexturePalettePair {
    pub texture_name: Name,
    pub palette_name: Option<Name>,
}


pub fn build_image_names(model: &Model) -> ImageNames {
    let mut image_names = HashMap::new();
    let mut taken_names = HashSet::new();

    for mat in &model.materials {
        let texture_name = match mat.texture_name {
            Some(n) => n,
            None => continue,
        };
        let palette_name = mat.palette_name;
        let pair = TexturePalettePair {
            texture_name: texture_name,
            palette_name: palette_name,
        };

        // Generate a name. Try to use the texture name, or
        // the texture name + a number if that is already taken.
        let mut desired_name = format!("{}", name::IdFmt(&texture_name));
        if taken_names.contains(&desired_name) {
            for i in 0.. {
                desired_name = format!("{}{}", name::IdFmt(&texture_name), i);
                if !taken_names.contains(&desired_name) {
                    break;
                }
            }
        }

        taken_names.insert(desired_name.clone());
        image_names.insert(pair, desired_name);
    }

    image_names
}
