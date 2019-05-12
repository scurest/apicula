use db::{Database, PaletteId, TextureId};
use glium::{texture::RawImage2d, Display, Texture2d};
use nds;
use std::collections::HashMap;

// TODO: move somewhere more important.
pub type ImageId = (TextureId, Option<PaletteId>);

/// Maintains cache of all the GL texture we use.
pub struct TextureCache {
    /// Caches GL texture for image ids.
    table: HashMap<ImageId, Texture2d>,
    /// Texture used for materials with no texture.
    white_texture_: Texture2d,
    /// Texture used when textures are missing, etc.
    error_texture_: Texture2d,
}

impl TextureCache {
    pub fn new(display: &Display) -> TextureCache {
        // 1x1 white texture
        let white_image = RawImage2d::from_raw_rgba(vec![255, 255, 255, 255u8], (1, 1));
        let white_texture_ = Texture2d::new(display, white_image).unwrap();

        // 1x1 magenta texture
        let error_image = RawImage2d::from_raw_rgba(vec![255, 0, 255, 255u8], (1, 1));
        let error_texture_ = Texture2d::new(display, error_image).unwrap();

        TextureCache {
            table: HashMap::with_capacity(10),
            white_texture_,
            error_texture_,
        }
    }

    pub fn lookup(&self, image_id: ImageId) -> &Texture2d {
        self.table.get(&image_id).unwrap_or(self.error_texture())
    }

    /// Ensure the given image_id is in the cache.
    pub fn create(&mut self, display: &Display, db: &Database, image_id: ImageId) {
        use std::collections::hash_map::Entry;
        match self.table.entry(image_id) {
            Entry::Occupied(_) => (),
            Entry::Vacant(ve) => {
                let texture = &db.textures[image_id.0];
                let palette = image_id.1.map(|pal_id| &db.palettes[pal_id]);
                let rgba = match nds::decode_texture(texture, palette) {
                    Ok(rgba) => rgba,
                    Err(_) => return,
                };
                let dim = texture.params.dim();
                let image = RawImage2d::from_raw_rgba_reversed(&rgba.0, dim);
                ve.insert(Texture2d::new(display, image).unwrap());
            }
        }
    }

    pub fn clear(&mut self) {
        self.table.clear();
    }

    pub fn white_texture(&self) -> &Texture2d {
        &self.white_texture_
    }

    pub fn error_texture(&self) -> &Texture2d {
        &self.error_texture_
    }
}
