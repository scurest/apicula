use nitro::{Texture, Palette};
use errors::Result;
use util::bits::BitField;
use util::cur::Cur;

pub fn decode(texture: &Texture, palette: Option<&Palette>) -> Result<Vec<u8>>
{
    let w = texture.params.width() as usize;
    let h = texture.params.height() as usize;
    let format = texture.params.format().0;
    let mut state = DecodeState {
        rgba: vec![0; 4 * w * h],
        idx: 0,
        bad_texture: false,
        bad_palette: false,
    };

    match format {
        7 => decode_format7(&mut state, texture),
        1 | 2 | 3 | 4 | 5 | 6 => {
            if palette.is_none() {
                bail!("texture is missing a required palette");
            }
            let palette = palette.unwrap();

            if format == 5 {
                decode_compressed(&mut state, texture, palette);
            } else {
                decode_paletted(&mut state, texture, palette);
            }
        }
        _ => { bail!("bad texture format (format={})", format); }
    }

    if state.bad_texture || state.bad_palette {
        warn!("texture/palette data is probably corrupt!");
        debug!("namely with texture={:?}, palette={:?}", texture.name, palette.map(|p| &p.name));
    }

    Ok(state.rgba)
}

/// RGBA data in the process of being decoded.
///
/// The purpose of this struct is that if an error occurs, the appropriate flag
/// can be set by the decoding loop and then a higher level can report an error
/// and still be able to use the partially decoded RGBA data.
struct DecodeState {
    /// RGBA pixel data (each pixel is four bytes).
    rgba: Vec<u8>,
    /// Index in `rgba` where to write the next RGBA value.
    idx: usize,
    /// Set if we got an OOB access when fetching a texel.
    bad_texture: bool,
    /// Set if we got an OOB access when fetching a palette color.
    bad_palette: bool,
}

impl DecodeState {
    fn write_pixel(&mut self, pixel: [u8; 4]) {
        assert!(self.idx + 3 < self.rgba.len());
        self.rgba[self.idx] = pixel[0];
        self.rgba[self.idx+1] = pixel[1];
        self.rgba[self.idx+2] = pixel[2];
        self.rgba[self.idx+3] = pixel[3];
        self.idx += 4;
    }
}

macro_rules! try_or_else {
    ($e:expr, $f:expr) => {
        match $e {
            Ok(x) => x,
            Err(_) => $f,
        }
    };
}

fn decode_format7(state: &mut DecodeState, texture: &Texture) {
    // Direct Color Texture
    // Holds actual 16-bit color values (no palette)

    let offset = texture.params.offset() as usize;
    let width = texture.params.width() as usize;
    let height = texture.params.height() as usize;

    let data =
        Cur::from_buf_pos(&texture.tex_data.texture_data, offset);

    for n in 0..(width * height) {
        let texel = try_or_else!(data.nth::<u16>(n),
            { state.bad_texture = true; return }
        );
        let alpha_bit = texel.bits(15,16);
        state.write_pixel(rgb555a5(
            texel,
            if alpha_bit == 0 { 0 } else { 31 },
        ));
    }
}


fn decode_paletted(state: &mut DecodeState, texture: &Texture, palette: &Palette) {
    let texture_off = texture.params.offset() as usize;
    let width = texture.params.width();
    let height = texture.params.height();
    let format = texture.params.format().0;
    let color0_is_transparent = texture.params.is_color0_transparent();
    let palette_off = palette.off as usize;

    let size = texture.params.format().byte_len((width, height));

    let data =
        Cur::from_buf_pos(&palette.tex_data.texture_data, texture_off);
    let pal_data =
        Cur::from_buf_pos(&palette.tex_data.palette_data, palette_off);

    match format {
        2 => {
            // 4-Color Palette Texture
            for n in 0..size {
                let x = try_or_else!(data.nth::<u8>(n),
                    { state.bad_texture = true; return }
                );
                for &v in &[x.bits(0,2), x.bits(2,4), x.bits(4,6), x.bits(6,8)] {
                    let rgb = try_or_else!(pal_data.nth::<u16>(v as usize),
                        { state.bad_palette = true; 0 }
                    );
                    let transparent = v == 0 && color0_is_transparent;
                    let a = if transparent { 0 } else { 31 };
                    state.write_pixel(rgb555a5(rgb, a));
                }
            }
        }
        3 => {
            // 16-Color Palette Texture
            for n in 0..size {
                let x = try_or_else!(data.nth::<u8>(n),
                    { state.bad_texture = true; return }
                );
                for &v in &[x.bits(0,4), x.bits(4,8)] {
                    let rgb = try_or_else!(pal_data.nth::<u16>(v as usize),
                        { state.bad_palette = true; 0 }
                    );
                    let transparent = v == 0 && color0_is_transparent;
                    let a = if transparent { 0 } else { 31 };
                    state.write_pixel(rgb555a5(rgb, a));
                }
            }
        }
        4 => {
            // 256-Color Palette Texture
            for n in 0..size {
                let v = try_or_else!(data.nth::<u8>(n),
                    { state.bad_texture = true; return }
                );
                let rgb = try_or_else!(pal_data.nth::<u16>(v as usize),
                    { state.bad_palette = true; 0 }
                );
                let transparent = v == 0 && color0_is_transparent;
                let a = if transparent { 0 } else { 31 };
                state.write_pixel(rgb555a5(rgb, a));
            }
        }
        1 => {
            // A3I5 Translucent Texture (3-bit Alpha, 5-bit Color Index)
            for n in 0..size {
                let x = try_or_else!(data.nth::<u8>(n),
                    { state.bad_texture = true; return }
                );
                let rgb = try_or_else!(pal_data.nth::<u16>(x.bits(0,5) as usize),
                    { state.bad_palette = true; 0 }
                );
                let a = a3_to_a5(x.bits(5,8));
                state.write_pixel(rgb555a5(rgb, a));
            }
        }
        6 => {
            // A5I3 Translucent Texture (5-bit Alpha, 3-bit Color Index)
            for n in 0..size {
                let x = try_or_else!(data.nth::<u8>(n),
                    { state.bad_texture = true; return }
                );
                let rgb = try_or_else!(pal_data.nth::<u16>(x.bits(0,3) as usize),
                    { state.bad_palette = true; 0 }
                );
                let a = a3_to_a5(x.bits(3,8));
                state.write_pixel(rgb555a5(rgb, a));
            }
        }
        _ => unreachable!(),
    }
}

fn decode_compressed(state: &mut DecodeState, texture: &Texture, palette: &Palette) {
    let texture_off = texture.params.offset() as usize;
    let width = texture.params.width() as usize;
    let height = texture.params.height() as usize;
    let palette_off = palette.off as usize;
    let num_blocks_x = width / 4;

    let data1 =
        Cur::from_buf_pos(&texture.tex_data.compressed_data1, texture_off);
    let data2 =
        Cur::from_buf_pos(&texture.tex_data.compressed_data2, texture_off/2);
    let pal_data =
        Cur::from_buf_pos(&palette.tex_data.palette_data, palette_off);

    for y in 0..height {
        for x in 0..width {
            let idx = num_blocks_x * (y/4) + (x/4);
            let block = try_or_else!(data1.nth::<u32>(idx),
                { state.bad_texture = true; return; }
            );
            let extra = try_or_else!(data2.nth::<u16>(idx),
                { state.bad_texture = true; return; }
            );

            let texel_off = 2 * (4 * (y%4) + (x%4)) as u32;
            let texel = block.bits(texel_off, texel_off+2);
            let mode = extra.bits(14,16);
            let pal_addr = (extra.bits(0,14) as usize) << 1;

            let pixel = {
                let mut color = |n| {
                    let rgb = try_or_else!(pal_data.nth::<u16>(pal_addr+n),
                        { state.bad_palette = true; 0 }
                    );
                    rgb555a5(rgb, 31)
                };

                match (mode, texel) {
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
                }
            };

            state.write_pixel(pixel);
        }
    }
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
