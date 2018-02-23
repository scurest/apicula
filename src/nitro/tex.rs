use std::rc::Rc;
use nitro::Name;
use nitro::info_block;
use util::bits::BitField;
use errors::Result;
use util::cur::Cur;

pub struct Texture {
    pub name: Name,
    pub params: TextureParameters,
    pub tex_data: Rc<TexData>,
}

pub struct Palette {
    pub name: Name,
    pub off: u32,
    pub tex_data: Rc<TexData>,
}

pub struct TexData {
    pub texture_data: Vec<u8>,
    pub palette_data: Vec<u8>,
    pub compressed_data1: Vec<u8>,
    pub compressed_data2: Vec<u8>,
}


pub fn read_tex(cur: Cur) -> Result<(Vec<Texture>, Vec<Palette>)> {
    fields!(cur, TEX0 {
        stamp: [u8; 4],
        section_size: u32,
        padding: u32,
        texture_data_size_shr_3: u16,
        texture_off: u16,
        padding: u32,
        texture_data_off: u32,
        padding: u32,
        compressed_data1_size_shr_3: u16,
        compressed_info_off: u16,
        padding: u32,
        compressed_data1_off: u32,
        compressed_data2_off: u32,
        padding: u32,
        palette_data_size_shr_3: u16,
        unknown: u16,
        palette_off: u32,
        palette_data_off: u32,
    });

    check!(stamp == b"TEX0")?;


    let texture_data_size = (texture_data_size_shr_3 as usize) << 3;
    let palette_data_size = (palette_data_size_shr_3 as usize) << 3;
    let compressed_data1_size = (compressed_data1_size_shr_3 as usize) << 3;
    let compressed_data2_size = compressed_data1_size / 2;

    let texture_data = (cur + texture_data_off).next_n_u8s(texture_data_size)?.to_vec();
    let palette_data = (cur + palette_data_off).next_n_u8s(palette_data_size)?.to_vec();
    let compressed_data1 = (cur + compressed_data1_off).next_n_u8s(compressed_data1_size)?.to_vec();
    let compressed_data2 = (cur + compressed_data2_off).next_n_u8s(compressed_data2_size)?.to_vec();

    let tex_data = TexData { texture_data, palette_data, compressed_data1, compressed_data2 };
    let tex_data = Rc::new(tex_data);


    let textures =
        info_block::read::<(u32, u32)>(cur + texture_off)?
        .map(|((params, _), name)| {
            debug!("texture: {:?}", name);
            let params = TextureParameters::from_u32(params);
            trace!("params: {:?}", params);
            Texture { name, params, tex_data: Rc::clone(&tex_data) }
        })
        .collect::<Vec<_>>();

    let palettes =
        info_block::read::<(u16, u16)>(cur + palette_off)?
        .map(|((off_shr_3, _), name)| {
            debug!("palette: {:?}", name);
            let off = (off_shr_3 as u32) << 3;
            Palette { name, off, tex_data: Rc::clone(&tex_data) }
        })
        .collect::<Vec<_>>();


    Ok((textures, palettes))
}

#[derive(Debug)]
pub struct TextureParameters {
    pub offset: u32,
    pub repeat_s: bool,
    pub repeat_t: bool,
    pub mirror_s: bool,
    pub mirror_t: bool,
    pub width: u32,
    pub height: u32,
    pub format: u8,
    pub is_color0_transparent: bool,
    pub texcoord_transform_mode: u8,
}

impl TextureParameters {
    pub fn from_u32(x: u32) -> TextureParameters {
        let offset = x.bits(0,16) << 3;
        let repeat_s = x.bits(16,17) != 0;
        let repeat_t = x.bits(17,18) != 0;
        let mirror_s = x.bits(18,19) != 0;
        let mirror_t = x.bits(19,20) != 0;
        let width = 8 << x.bits(20,23);
        let height = 8 << x.bits(23,26);
        let format = x.bits(26,29) as u8;
        let is_color0_transparent = x.bits(29,30) != 0;
        let texcoord_transform_mode = x.bits(30,32) as u8;

        TextureParameters {
            offset, repeat_s, repeat_t, mirror_s, mirror_t,
            width, height, format, is_color0_transparent,
            texcoord_transform_mode,
        }
    }
}
