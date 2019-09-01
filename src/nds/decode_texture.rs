use nitro::{Texture, Palette};
use errors::Result;
use util::bits::BitField;
use util::cur::Cur;

/// Pixel data stored in R8G8B8A8 format.
pub struct RGBABuf(pub Vec<u8>);

impl RGBABuf {
    pub fn for_dimensions((width, height): (u32, u32)) -> RGBABuf {
        let (w, h) = (width as usize, height as usize);
        RGBABuf(Vec::with_capacity(4 * w * h))
    }

    fn pixel(&mut self, pixel: [u8; 4]) {
        self.0.extend_from_slice(&pixel);
    }
}

/// Decodes a texture/palette combo to RGBA.
pub fn decode_texture(texture: &Texture, palette: Option<&Palette>) -> Result<RGBABuf> {
    let params = texture.params;
    let (w, h) = params.dim();
    let requires_palette = params.format().desc().requires_palette;

    if requires_palette && palette.is_none() {
        bail!("texture required a palette");
    }

    let mut buf = RGBABuf::for_dimensions((w, h));

    use super::TextureFormat as F;
    match params.format() {
        F(0) => bail!("texture had format 0"),
        F(1) => decode_format1(&mut buf, texture, palette.unwrap()),
        F(2) => decode_format2(&mut buf, texture, palette.unwrap()),
        F(3) => decode_format3(&mut buf, texture, palette.unwrap()),
        F(4) => decode_format4(&mut buf, texture, palette.unwrap()),
        F(5) => decode_format5(&mut buf, texture, palette.unwrap()),
        F(6) => decode_format6(&mut buf, texture, palette.unwrap()),
        F(7) => decode_format7(&mut buf, texture),
        _ => unreachable!(),
    }

    Ok(buf)
}

fn decode_format1(buf: &mut RGBABuf, tex: &Texture, pal: &Palette) {
    // A3I5 Translucent Texture (3-bit Alpha, 5-bit Color Index)
    let (w, h) = tex.params.dim();
    let num_texels = (w * h) as usize;
    let data = &tex.data1[..];
    let pal_cur = Cur::from_buf_pos(&pal.pal_block[..], pal.off as usize);
    for n in 0..num_texels {
        let x = data[n];
        let rgb = pal_cur.nth::<u16>(x.bits(0,5) as usize).unwrap_or(0);
        let a = a3_to_a5(x.bits(5,8));
        buf.pixel(rgb555a5(rgb, a));
    }
}

fn decode_format2(buf: &mut RGBABuf, tex: &Texture, pal: &Palette) {
    // 4-Color Palette Texture
    let (w, h) = tex.params.dim();
    let num_bytes = tex.params.format().byte_len((w, h));
    let color0_is_transparent = tex.params.is_color0_transparent();
    let data = &tex.data1;
    let pal_cur = Cur::from_buf_pos(&pal.pal_block[..], pal.off as usize);
    for n in 0..num_bytes {
        let x = data[n];

        macro_rules! do_pixel {
            ($lo:expr, $hi:expr) => {
                let u = x.bits($lo, $hi);
                let rgb = pal_cur.nth::<u16>(u as usize).unwrap_or(0);
                let transparent = u == 0 && color0_is_transparent;
                let a = if transparent { 0 } else { 31 };
                buf.pixel(rgb555a5(rgb, a));
            };
        }

        do_pixel!(0, 2);
        do_pixel!(2, 4);
        do_pixel!(4, 6);
        do_pixel!(6, 8);
    }
}

fn decode_format3(buf: &mut RGBABuf, tex: &Texture, pal: &Palette) {
    // 16-Color Palette Texture
    let (w, h) = tex.params.dim();
    let num_bytes = tex.params.format().byte_len((w, h));
    let color0_is_transparent = tex.params.is_color0_transparent();
    let data = &tex.data1;
    let pal_cur = Cur::from_buf_pos(&pal.pal_block[..], pal.off as usize);
    for n in 0..num_bytes {
        let x = data[n];

        macro_rules! do_pixel {
            ($lo:expr, $hi:expr) => {
                let u = x.bits($lo, $hi);
                let rgb = pal_cur.nth::<u16>(u as usize).unwrap_or(0);
                let transparent = u == 0 && color0_is_transparent;
                let a = if transparent { 0 } else { 31 };
                buf.pixel(rgb555a5(rgb, a));
            };
        }

        do_pixel!(0, 4);
        do_pixel!(4, 8);
    }
}

fn decode_format4(buf: &mut RGBABuf, tex: &Texture, pal: &Palette) {
    // 256-Color Palette Texture
    let (w, h) = tex.params.dim();
    let num_bytes = tex.params.format().byte_len((w, h));
    let color0_is_transparent = tex.params.is_color0_transparent();
    let data = &tex.data1;
    let pal_cur = Cur::from_buf_pos(&pal.pal_block[..], pal.off as usize);
    for n in 0..num_bytes {
        let x = data[n];

        let rgb = pal_cur.nth::<u16>(x as usize).unwrap_or(0);
        let transparent = x == 0 && color0_is_transparent;
        let a = if transparent { 0 } else { 31 };
        buf.pixel(rgb555a5(rgb, a));
    }
}

fn decode_format5(buf: &mut RGBABuf, tex: &Texture, pal: &Palette) {
    // Block-compressed Texture
    let (w, h) = tex.params.dim();
    let num_blocks_x = w / 4;
    let num_blocks = (w * h / 16) as usize;

    let data1 = Cur::new(&tex.data1).next_n::<u32>(num_blocks).unwrap();
    let data2 = Cur::new(&tex.data2).next_n::<u16>(num_blocks).unwrap();
    let pal_cur = Cur::from_buf_pos(&pal.pal_block[..], pal.off as usize);

    for y in 0..h {
        for x in 0..w {
            // Find the block containing (x, y)
            let block_idx = num_blocks_x * (y/4) + (x/4);
            let block = data1.nth(block_idx as usize);
            let extra = data2.nth(block_idx as usize);

            // Find the bits for this texel within the block
            let texel_off = 2 * (4 * (y%4) + (x%4)) as u32;
            let texel = block.bits(texel_off, texel_off+2);

            let mode = extra.bits(14,16);
            let pal_addr = (extra.bits(0,14) as usize) << 1;

            let pixel = {
                let color = |n| {
                    let rgb = pal_cur.nth::<u16>(pal_addr+n).unwrap_or(0);
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

            buf.pixel(pixel);
        }
    }
}

fn decode_format6(buf: &mut RGBABuf, tex: &Texture, pal: &Palette) {
    // A5I3 Translucent Texture (5-bit Alpha, 3-bit Color Index)
    let (w, h) = tex.params.dim();
    let num_texels = (w * h) as usize;
    let data = &tex.data1[..];
    let pal_cur = Cur::from_buf_pos(&pal.pal_block[..], pal.off as usize);
    for n in 0..num_texels {
        let x = data[n];
        let rgb = pal_cur.nth::<u16>(x.bits(0,3) as usize).unwrap_or(0);
        let a = a3_to_a5(x.bits(3,8));
        buf.pixel(rgb555a5(rgb, a));
    }
}

fn decode_format7(buf: &mut RGBABuf, tex: &Texture) {
    // Direct Color Texture
    // Holds actual 16-bit color values (no palette)
    let (w, h) = tex.params.dim();
    let num_texels = (w * h) as usize;
    let data = Cur::new(&tex.data1).next_n::<u16>(num_texels).unwrap();
    for n in 0..num_texels {
        let texel = data.nth(n);
        let alpha_bit = texel.bits(15,16);
        buf.pixel(rgb555a5(
            texel,
            if alpha_bit == 0 { 0 } else { 31 },
        ));
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
