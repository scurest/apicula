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

/// Build a mapping from images (texture/palette pairs) to
/// readable names.
///
/// We try to name every image by its texture name, but these
/// can collide. This function picks a unique name for every
/// image by appending numbers to disambiguate collisions.
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

        let desired_name = format!("{}", name::IdFmt(&texture_name));
        let name = find_free_name(&mut taken_names, desired_name);
        image_names.insert(pair, name);
    }

    image_names
}

/// Picks the first name from
///
/// * `"{desired_name}"`
/// * `"{desired_name}1"`
/// * `"{desired_name}2"`
/// * ...
///
/// which is not taken, marks it as taken, and returns it.
fn find_free_name(taken_names: &mut HashSet<String>, desired_name: String) -> String {
    let chosen_name = if !taken_names.contains(&desired_name) {
        desired_name
    } else {
        let mut name = String::new();
        for i in 1.. {
            name = format!("{}{}", desired_name, i);
            if !taken_names.contains(&name) {
                break;
            }
        }
        name
    };
    taken_names.insert(chosen_name.clone());
    chosen_name
}
