use nitro::{Texture, Palette};
use errors::Result;
use util::bits::BitField;
use util::cur::Cur;

pub fn decode(texture: &Texture, palette: Option<&Palette>) -> Result<Vec<u8>>
{
    let format = texture.params.format;

    if format == 7 {
        return Ok(decode_format7(texture));
    }

    if palette.is_none() {
        bail!("texture is missing palette");
    }
    let palette = palette.unwrap();

    Ok(match format {
        1 | 2 | 3 | 4 | 6 => decode_paletted(texture, palette),
        5 => decode_compressed(texture, palette),
        _ => { bail!("bad texture format (format={})", format); }
    })
}

/// This macro is used when fetching texels. We don't want to return an `Err`
/// since we might have _some_ usable data, so instead when a fetch fails we
/// return what RGBA we've built so far.
macro_rules! try_fetch{
    ($fetch:expr, $ret:expr) => {
        match $fetch {
            Ok(x) => x,
            Err(_) => return $ret,
        }
    };
}

fn decode_format7(texture: &Texture) -> Vec<u8> {
    // Direct Color Texture
    // Holds actual 16-bit color values (no palette)

    let offset = texture.params.offset as usize;
    let width = texture.params.width as usize;
    let height = texture.params.height as usize;

    let data =
        Cur::from_buf_pos(&texture.tex_data.texture_data, offset);

    let mut pixels = vec![0u8; 4 * width * height]; // 4 bytes (RGBA) for every texel
    let mut i = 0;

    for n in 0..(width * height) {
        let texel = try_fetch!(data.nth::<u16>(n), pixels);

        let alpha_bit = texel.bits(15,16);
        write_pixel(&mut pixels, &mut i, rgb555a5(
            texel,
            if alpha_bit == 0 { 0 } else { 31 },
        ));
    }

    pixels
}


fn decode_paletted(texture: &Texture, palette: &Palette) -> Vec<u8>
{
    let texture_off = texture.params.offset as usize;
    let width = texture.params.width as usize;
    let height = texture.params.height as usize;
    let format = texture.params.format;
    let color0_is_transparent = texture.params.is_color0_transparent;
    let palette_off = palette.off as usize;

    static BPPS: [u8; 7] = [0, 8, 2, 4, 8, 0, 8];
    let bpp = BPPS[format as usize] as usize;
    let size = width * height * bpp / 8; // number of bytes of texture data

    let data =
        Cur::from_buf_pos(&palette.tex_data.texture_data, texture_off);
    let pal_data =
        Cur::from_buf_pos(&palette.tex_data.palette_data, palette_off);

    let mut pixels = vec![0u8; 4 * width * height]; // 4 bytes (RGBA) for every texel
    let mut i = 0;

    match format {
        2 => {
            // 4-Color Palette Texture
            for n in 0..size {
                let x = try_fetch!(data.nth::<u8>(n), pixels);
                for &v in &[x.bits(0,2), x.bits(2,4), x.bits(4,6), x.bits(6,8)] {
                    let transparent = v == 0 && color0_is_transparent;
                    write_pixel(&mut pixels, &mut i, rgb555a5(
                        pal_data.nth::<u16>(v as usize).unwrap_or(0),
                        if transparent { 0 } else { 31 },
                    ));
                }
            }
        }
        3 => {
            // 16-Color Palette Texture
            for n in 0..size {
                let x = try_fetch!(data.nth::<u8>(n), pixels);
                for &v in &[x.bits(0,4), x.bits(4,8)] {
                    let transparent = v == 0 && color0_is_transparent;
                    write_pixel(&mut pixels, &mut i, rgb555a5(
                        pal_data.nth::<u16>(v as usize).unwrap_or(0),
                        if transparent { 0 } else { 31 },
                    ));
                }
            }
        }
        4 => {
            // 256-Color Palette Texture
            for n in 0..size {
                let v = try_fetch!(data.nth::<u8>(n), pixels);
                let transparent = v == 0 && color0_is_transparent;
                write_pixel(&mut pixels, &mut i, rgb555a5(
                    pal_data.nth::<u16>(v as usize).unwrap_or(0),
                    if transparent { 0 } else { 31 },
                ));
            }
        }
        1 => {
            // A3I5 Translucent Texture (3-bit Alpha, 5-bit Color Index)
            for n in 0..size {
                let x = try_fetch!(data.nth::<u8>(n), pixels);
                write_pixel(&mut pixels, &mut i, rgb555a5(
                    pal_data.nth::<u16>(x.bits(0,5) as usize).unwrap_or(0),
                    a3_to_a5(x.bits(5,8)),
                ));
            }
        }
        6 => {
            // A5I3 Translucent Texture (5-bit Alpha, 3-bit Color Index)
            for n in 0..size {
                let x = try_fetch!(data.nth::<u8>(n), pixels);
                write_pixel(&mut pixels, &mut i, rgb555a5(
                    pal_data.nth::<u16>(x.bits(0,3) as usize).unwrap_or(0),
                    x.bits(3,8),
                ));
            }
        }
        _ => unreachable!(),
    }

    pixels
}

pub fn decode_compressed(texture: &Texture, palette: &Palette) -> Vec<u8> {
    let texture_off = texture.params.offset as usize;
    let width = texture.params.width as usize;
    let height = texture.params.height as usize;
    let palette_off = palette.off as usize;
    let num_blocks_x = width / 4;

    let data1 =
        Cur::from_buf_pos(&texture.tex_data.compressed_data1, texture_off);
    let data2 =
        Cur::from_buf_pos(&texture.tex_data.compressed_data2, texture_off/2);
    let pal_data =
        Cur::from_buf_pos(&palette.tex_data.palette_data, palette_off);

    let mut pixels = vec![0u8; 4*width*height];
    let mut i = 0;

    for y in 0..height {
        for x in 0..width {
            let idx = num_blocks_x * (y/4) + (x/4);
            let block = try_fetch!(data1.nth::<u32>(idx), pixels);
            let extra = try_fetch!(data2.nth::<u16>(idx), pixels);

            let texel_off = 2 * (4 * (y%4) + (x%4)) as u32;
            let texel = block.bits(texel_off, texel_off+2);

            let pal_addr = (extra.bits(0,14) as usize) << 1;
            let color = |n| {
                rgb555a5(pal_data.nth::<u16>(pal_addr+n).unwrap_or(0), 31)
            };

            let mode = extra.bits(14,16);

            let color = match (mode, texel) {
                (0, 0) => color(0),
                (0, 1) => color(1),
                (0, 2) => color(2),
                (0, 3) => [0, 0, 0, 0],

                (1, 0) => color(0),
                (1, 1) => color(1),
                (1, 2) => avg(color(0), color(1)),
                (1, 3) => [0, 0, 0, 0],

                (2, 0) => color(0),
                (2, 1) => color(1),
                (2, 2) => color(2),
                (2, 3) => color(3),

                (3, 0) => color(0),
                (3, 1) => color(1),
                (3, 2) => avg358(color(1), color(0)),
                (3, 3) => avg358(color(0), color(1)),

                _ => unreachable!(),
            };
            write_pixel(&mut pixels, &mut i, color);
        }
    }

    pixels
}

fn write_pixel(pixels: &mut [u8], i: &mut usize, pixel: [u8; 4]) {
    assert!(*i + 3 < pixels.len());
    pixels[*i] = pixel[0];
    pixels[*i+1] = pixel[1];
    pixels[*i+2] = pixel[2];
    pixels[*i+3] = pixel[3];
    *i += 4;
}

/// Converts RGB555 color and A5 alpha into RGBA8888.
fn rgb555a5(rgb555: u16, a5: u8) -> [u8; 4] {
    let r5 = rgb555.bits(0,5) as u8;
    let g5 = rgb555.bits(5,10) as u8;
    let b5 = rgb555.bits(10,15) as u8;
    let r8 = extend_5bit_to_8bit(r5);
    let g8 = extend_5bit_to_8bit(g5);
    let b8 = extend_5bit_to_8bit(b5);
    let a8 = extend_5bit_to_8bit(a5);
    [r8, g8, b8, a8]
}

fn a3_to_a5(x: u8) -> u8 {
    (x << 2) | (x >> 1)
}

fn extend_5bit_to_8bit(x: u8) -> u8 {
    (x << 3) | (x >> 2)
}

/// (c1 + c2) / 2
fn avg(c1: [u8; 4], c2: [u8; 4]) -> [u8; 4] {
    [
        ((c1[0] as u32 + c2[0] as u32) / 2) as u8,
        ((c1[1] as u32 + c2[1] as u32) / 2) as u8,
        ((c1[2] as u32 + c2[2] as u32) / 2) as u8,
        ((c1[3] as u32 + c2[3] as u32) / 2) as u8,
    ]
}

/// (3*c1 + 5*c2) / 8
fn avg358(c1: [u8; 4], c2: [u8; 4]) -> [u8; 4] {
    [
        ((3*c1[0] as u32 + 5*c2[0] as u32) / 8) as u8,
        ((3*c1[1] as u32 + 5*c2[1] as u32) / 8) as u8,
        ((3*c1[2] as u32 + 5*c2[2] as u32) / 8) as u8,
        ((3*c1[3] as u32 + 5*c2[3] as u32) / 8) as u8,
    ]
}
