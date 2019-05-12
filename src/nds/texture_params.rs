use super::TextureFormat;
use util::bits::BitField;

#[derive(Copy, Clone)]
pub struct TextureParams(pub u32);

impl TextureParams {
    pub fn offset(self) -> u32 {
        self.0.bits(0, 16) << 3
    }
    pub fn repeat_s(self) -> bool {
        self.0.bits(16, 17) != 0
    }
    pub fn repeat_t(self) -> bool {
        self.0.bits(17, 18) != 0
    }
    pub fn mirror_s(self) -> bool {
        self.0.bits(18, 19) != 0
    }
    pub fn mirror_t(self) -> bool {
        self.0.bits(19, 20) != 0
    }
    pub fn width(self) -> u32 {
        8 << self.0.bits(20, 23)
    }
    pub fn height(self) -> u32 {
        8 << self.0.bits(23, 26)
    }
    pub fn dim(self) -> (u32, u32) {
        (self.width(), self.height())
    }
    pub fn format(self) -> TextureFormat {
        TextureFormat(self.0.bits(26, 29) as u8)
    }
    pub fn is_color0_transparent(self) -> bool {
        self.0.bits(29, 30) != 0
    }
    pub fn texcoord_transform_mode(self) -> u8 {
        self.0.bits(30, 32) as u8
    }
}

// NOTE: there is a u32 for texture parameters stored in both the texture itself
// and the model's material. I believe they are or-ed together to get the final
// parameters, with the texture storing innate properties (eg. format, width,
// height) and the material storing ephermeral properties (eg. repeat, mirror,
// transform mode).

impl std::fmt::Debug for TextureParams {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("TextureParams")
            .field("dim", &(self.width(), self.height()))
            .field("format", &self.format().0)
            .field("offset", &self.offset())
            .field("repeat", &(self.repeat_s(), self.repeat_t()))
            .field("mirror", &(self.mirror_s(), self.mirror_t()))
            .field("color0_transparent", &self.is_color0_transparent())
            .field("texcoord_transform_mode", &self.texcoord_transform_mode())
            .finish()
    }
}
