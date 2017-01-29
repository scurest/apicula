use nitro::mdl::Material;
use nitro::name::IdFmt;
use nitro::name::Name;
use nitro::tex::PaletteInfo;
use nitro::tex::Tex;
use nitro::tex::TextureInfo;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct TexPalPair(pub Name, pub Option<Name>);

impl TexPalPair {
    pub fn from_material(mat: &Material) -> Option<TexPalPair> {
        mat.texture_name
            .map(|texture_name| TexPalPair(texture_name, mat.palette_name))
    }
}

// This is used by UniqueNamer to generate image names,
// see the `convert` module.
impl ToString for TexPalPair {
    fn to_string(&self) -> String {
        format!("{}", IdFmt(&self.0))
    }
}

/// Search `texs` for a `TextureInfo` whose name matches the texture
/// name in pair.
///
/// If one is found, the `Tex` it was contained in and any matching
/// `PaletteInfo` in that same `Tex` is also returned. Otherwise,
/// returns `None`.
pub fn find_tex<'a, 'b: 'a>(texs: &'a [Tex<'b>], pair: TexPalPair)
-> Option<(&'a Tex<'b>, &'a TextureInfo, Option<&'a PaletteInfo>)> {
    let texture_name = pair.0;
    let palette_name = pair.1;

    for tex in texs {
        let res = tex.texinfo.iter().find(|info| texture_name == info.name);
        if let Some(texinfo) = res {
            let pal = palette_name.and_then(|palname| {
                tex.palinfo.iter().find(|info| info.name == palname)
            });
            return Some((tex, texinfo, pal));
        }
    }
    None
}
