use errors::Result;
use nitro::info_block;
use nitro::tex::PaletteInfo;
use nitro::tex::Tex;
use nitro::tex::TextureInfo;
use nitro::tex::TextureParameters;
use util::cur::Cur;

pub fn read_tex(cur: Cur) -> Result<Tex> {
    fields!(cur, tex0 {
        stamp: [u8; 4],
        section_size: u32,
        padding: u32,
        texture_data_size_shr_3: u16,
        texture_info_off: u16,
        padding: u32,
        texture_data_off: u32,
        padding: u32,
        compressed_texture_data_size_shr_3: u16,
        compressed_texture_info_off: u16,
        padding: u32,
        compressed_texture_data_off: u32,
        compressed_texture_extra_off: u32,
        padding: u32,
        palette_data_size_shr_3: u16,
        unknown: u16,
        palette_info_off: u32,
        palette_data_off: u32,
    });
    check!(stamp == &b"TEX0"[..])?;

    let texture_data_size = (texture_data_size_shr_3 as usize) << 3;
    let compressed_texture_data_size = (compressed_texture_data_size_shr_3 as usize) << 3;
    let compressed_texture_extra_size = compressed_texture_data_size / 2;
    let palette_data_size = (palette_data_size_shr_3 as usize) << 3;

    let texinfo = read_tex_info((cur + texture_info_off as usize)?)?;
    let palinfo = read_pal_info((cur + palette_info_off as usize)?)?;

    let texture_data = (cur + texture_data_off as usize)?
        .next_n_u8s(texture_data_size)?;
    let compressed_texture_data = (cur + compressed_texture_data_off as usize)?
        .next_n_u8s(compressed_texture_data_size)?;
    let compressed_texture_extra_data = (cur + compressed_texture_extra_off as usize)?
        .next_n_u8s(compressed_texture_extra_size)?;
    let palette_data = (cur + palette_data_off as usize)?
        .next_n_u8s(palette_data_size)?;

    Ok(Tex {
        texinfo: texinfo,
        palinfo: palinfo,
        texture_data: texture_data,
        compressed_texture_data: compressed_texture_data,
        compressed_texture_extra_data: compressed_texture_extra_data,
        palette_data: palette_data,
    })
}

fn read_pal_info(cur: Cur) -> Result<Vec<PaletteInfo>> {
    Ok(info_block::read::<(u16, u16)>(cur)?
        .map(|((off_shr_3, _), name)| {
            PaletteInfo {
                name: name,
                off: (off_shr_3 as usize) << 3,
            }
        })
        .collect()
    )
}

fn read_tex_info(cur: Cur) -> Result<Vec<TextureInfo>> {
    Ok(info_block::read::<(u32, u32)>(cur)?
        .map(|((params, _), name)| {
            TextureInfo {
                name: name,
                params: TextureParameters(params),
            }
        })
        .collect()
    )
}
