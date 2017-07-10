//! Nitro texture and palette data.

use nitro::name::Name;
use util::bits::BitField;

pub mod image;
pub mod texpal;
mod read;

pub use self::read::read_tex;

#[derive(Debug, Clone)]
pub struct Tex<'a> {
    pub texinfo: Vec<TextureInfo>,
    pub palinfo: Vec<PaletteInfo>,
    pub texture_data: &'a [u8],
    pub compressed_texture_data: &'a [u8],
    pub compressed_texture_extra_data: &'a [u8],
    pub palette_data: &'a [u8],
}

#[derive(Debug, Clone)]
pub struct TextureInfo {
    pub name: Name,
    pub params: TextureParameters,
}

#[derive(Debug, Clone)]
pub struct PaletteInfo {
    pub name: Name,
    pub off: usize,
}

#[derive(Debug, Copy, Clone)]
pub struct TextureParameters(pub u32);

impl TextureParameters {
    pub fn offset(self) -> usize { (self.0.bits(0,16) as usize) << 3 }
    pub fn repeat_s(self) -> bool { self.0.bits(16,17) != 0 }
    pub fn repeat_t(self) -> bool { self.0.bits(17,18) != 0 }
    pub fn mirror_s(self) -> bool { self.0.bits(18,19) != 0 }
    pub fn mirror_t(self) -> bool { self.0.bits(19,20) != 0 }
    pub fn width(self) -> u32 { 8 << self.0.bits(20,23) }
    pub fn height(self) -> u32 { 8 << self.0.bits(23,26) }
    pub fn format(self) -> u32 { self.0.bits(26,29) }
    pub fn is_color0_transparent(self) -> bool { self.0.bits(29,30) != 0 }
    pub fn texcoord_transform_mode(self) -> u32 { self.0.bits(30,32) }

    pub fn is_direct_color(self) -> bool { self.format() == 7 }
}
