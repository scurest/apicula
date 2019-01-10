//! NDS texture formats info.

use super::TextureParams;

#[derive(Copy, Clone)]
pub struct TextureFormat(pub u8);

pub enum Alpha {
    // Alpha = 1
    Opaque,
    // Alpha = 0 or 1
    Transparent,
    // 0 <= Alpha <= 1
    Translucent,
}

impl TextureFormat {
    pub fn desc(self) -> &'static FormatDesc {
        &DESCS[self.0 as usize]
    }

    /// How many bytes a texture of the given size takes up in this format.
    pub fn byte_len(self, (width, height): (u32, u32)) -> usize {
        let bit_len = width * height * self.desc().bpp as u32;
        bit_len as usize / 8
    }

    /// Whether this format can have transparent or translucent texels when
    /// drawn with the given parameters.
    pub fn alpha_type(self, params: TextureParams) -> Alpha {
        let is_color0_transparent = params.is_color0_transparent();
        match self.desc().alpha_desc {
            AlphaDesc::Opaque => Alpha::Opaque,
            AlphaDesc::Transparent => Alpha::Transparent,
            AlphaDesc::Translucent => Alpha::Translucent,
            AlphaDesc::TransparentDependingOnParams => {
                if is_color0_transparent {
                    Alpha::Transparent
                } else {
                    Alpha::Opaque
                }
            }
        }
    }
}

/// Describes properties of an NDS texture format.
pub struct FormatDesc {
    pub name: &'static str,
    pub requires_palette: bool,
    pub bpp: u8,
    pub alpha_desc: AlphaDesc,
}

pub enum AlphaDesc {
    Opaque,
    Transparent,
    TransparentDependingOnParams,
    Translucent,
}

pub static DESCS: [FormatDesc; 8] = [
    // 0, not really a real texture format
    FormatDesc {
        name: "None",
        requires_palette: false,
        bpp: 0,
        alpha_desc: AlphaDesc::Opaque,
    },
    // 1
    FormatDesc {
        name: "A3I5 Translucent Texture",
        requires_palette: true,
        bpp: 8,
        alpha_desc: AlphaDesc::Translucent,
    },
    // 2
    FormatDesc {
        name: "4-Color Palette Texture",
        requires_palette: true,
        bpp: 2,
        alpha_desc: AlphaDesc::TransparentDependingOnParams,
    },
    // 3
    FormatDesc {
        name: "16-Color Palette Texture",
        requires_palette: true,
        bpp: 4,
        alpha_desc: AlphaDesc::TransparentDependingOnParams,
    },
    // 4
    FormatDesc {
        name: "256-Color Palette Texture",
        requires_palette: true,
        bpp: 8,
        alpha_desc: AlphaDesc::TransparentDependingOnParams,
    },
    // 5
    FormatDesc {
        name: "Block-Compressed Texture",
        requires_palette: true,
        bpp: 2,
        alpha_desc: AlphaDesc::Transparent,
    },
    // 6
    FormatDesc {
        name: "A5I3 Translucent Texture",
        requires_palette: true,
        bpp: 8,
        alpha_desc: AlphaDesc::Translucent,
    },
    // 7
    FormatDesc {
        name: "Direct RGBA Texture",
        requires_palette: false,
        bpp: 16,
        alpha_desc: AlphaDesc::Transparent,
    },
];
