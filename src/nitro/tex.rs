use std::rc::Rc;
use nitro::Name;
use nitro::info_block;
use errors::Result;
use util::cur::Cur;
use nds::TextureParams;

pub struct Texture {
    pub name: Name,
    pub params: TextureParams,
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
            let params = TextureParams(params);
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
