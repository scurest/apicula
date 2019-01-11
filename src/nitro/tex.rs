use std::rc::Rc;
use nitro::Name;
use nitro::info_block;
use errors::Result;
use util::cur::Cur;
use nds::{TextureParams, TextureFormat};

pub struct Texture {
    pub name: Name,
    pub params: TextureParams,
    pub data1: Vec<u8>,
    /// Only used by block-compressed textures.
    pub data2: Vec<u8>,
}

pub struct Palette {
    pub name: Name,
    pub off: u32,
    /// Since we don't know how large a palette is
    pub pal_block: Rc<Box<[u8]>>,
}

pub fn read_tex(cur: Cur) -> Result<(Vec<Texture>, Vec<Palette>)> {
    fields!(cur, TEX0 {
        stamp: [u8; 4],
        section_size: u32,
        _unknown: u32,
        tex_block_len_shr_3: u16,
        texture_off: u16,
        _unknown: u32,
        tex_block_off: u32,
        _unknown: u32,
        compressed_block_len_shr_3: u16,
        compressed_info_off: u16,
        _unknown: u32,
        compressed1_block_off: u32,
        compressed2_block_off: u32,
        _unknown: u32,
        pal_block_len_shr_3: u16,
        _unknown: u16,
        palette_off: u32,
        pal_block_off: u32,
    });

    check!(stamp == b"TEX0")?;

    // Stores palette data.
    let pal_block_len = (pal_block_len_shr_3 as usize) << 3;
    let pal_block = (cur + pal_block_off).next_n_u8s(pal_block_len)?.to_vec().into_boxed_slice();
    let pal_block = Rc::new(pal_block);

    // Stores regular texture data.
    let tex_cur = cur + tex_block_off;
    // Stores texture data for block-compressed formats. One block compressed
    // texture consists of two parallel arrays, one in the compressed1 block and
    // one half the length in the compressed2 block.
    let compressed1_cur = cur + compressed1_block_off;
    let compressed2_cur = cur + compressed2_block_off;


    let textures =
        info_block::read::<(u32, u32)>(cur + texture_off)?
        .map(|((params, _unknown), name)| {
            debug!("texture: {:?}", name);

            let params = TextureParams(params);
            trace!("params: {:?}", params);
            let off = params.offset();
            let len = params.format().byte_len((params.width(), params.height()));

            let (data1, data2);
            match params.format() {
                TextureFormat(5) => {
                    data1 = (compressed1_cur + off).next_n_u8s(len)?.to_vec();
                    data2 = (compressed2_cur + off/2).next_n_u8s(len/2)?.to_vec();
                },
                _ => {
                    data1 = (tex_cur + off).next_n_u8s(len)?.to_vec();
                    data2 = vec![];
                }
            }

            Ok(Texture { name, params, data1, data2 })
        })
        .collect::<Result<Vec<_>>>()?;

    let palettes =
        info_block::read::<(u16, u16)>(cur + palette_off)?
        .map(|((off_shr_3, _unknown), name)| {
            debug!("palette: {:?}", name);
            let off = (off_shr_3 as u32) << 3;
            Palette { name, off, pal_block: Rc::clone(&pal_block) }
        })
        .collect::<Vec<_>>();


    Ok((textures, palettes))
}
