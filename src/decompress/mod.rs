//! Decompress NDS data.
//!
//! The compressions methods are the LZ77 methods included in the NDS bios.
//!
//! See: http://problemkaputt.de/gbatek.htm#biosdecompressionfunctions
//! See: DSDecmp (https://github.com/Barubary/dsdecmp)

use std::{fmt, error, result};
use util::bits::BitField;
use util::cur::{self, Cur};

pub struct DecompressResult<'a> {
    /// The decompressed data.
    pub data: Vec<u8>,
    /// A cursor at the end of the compressed data stream.
    pub end_cur: Cur<'a>,
}

/// Try to decompress data at `cur`.
pub fn decompress(cur: Cur) -> Result<DecompressResult> {
    match cur.peek::<u8>() {
        Ok(0x10) => de_lz77_0x10(cur),
        Ok(0x11) => de_lz77_0x11(cur),
        _ => Err(Error::DecompressFailed),
    }
}

fn de_lz77_0x10(mut cur: Cur) -> Result<DecompressResult> {
    let header = cur.next::<u32>()?;
    let ty = header.bits(0,8);
    if ty != 0x10 {
        return Err(Error::DecompressFailed);
    }
    let mut decompressed_size = header.bits(8, 32) as usize;
    if decompressed_size == 0 {
        decompressed_size = cur.next::<u32>()? as usize;
    }

    // Too short to contain anything interesting
    if decompressed_size < 40 {
        return Err(Error::DecompressFailed);
    }
    // Too big (> 4 MiB)
    if decompressed_size > (1 << 19) * 4 {
        return Err(Error::DecompressFailed);
    }

    let mut out = Vec::with_capacity(decompressed_size);

    while out.len() < decompressed_size {
        let mut flags = cur.next::<u8>()?;
        for _ in 0..8 {
            let compressed = flags & 0x80 != 0;
            flags <<= 1;
            if !compressed {
                // Uncompressed byte
                out.push(cur.next::<u8>()?);
            } else {
                // LZ backreference
                let (ofs_sub_1, n_sub_3) = {
                    let x = cur.next::<u16>()?.swap_bytes(); // stored big-endian
                    (x.bits(0, 12) as usize, x.bits(12, 16) as usize)
                };
                let (ofs, n) = (ofs_sub_1 + 1, n_sub_3 + 3);

                if out.len() + n > decompressed_size { // too much data
                    return Err(Error::DecompressFailed);
                }
                if out.len() < ofs { // not enough data
                    return Err(Error::DecompressFailed);
                }

                for _ in 0 .. n {
                    let x = out[out.len() - ofs];
                    out.push(x);
                }
            }

            if out.len() >= decompressed_size {
                break;
            }
        }
    }

    Ok(DecompressResult {
        data: out,
        end_cur: cur,
    })
}

fn de_lz77_0x11(mut cur: Cur) -> Result<DecompressResult> {
    let header = cur.next::<u32>()?;
    let ty = header.bits(0,8);
    if ty != 0x11 {
        return Err(Error::DecompressFailed);
    }
    let mut decompressed_size = header.bits(8, 32) as usize;
    if decompressed_size == 0 {
        decompressed_size = cur.next::<u32>()? as usize;
    }

    // Too short to contain anything interesting
    if decompressed_size < 40 {
        return Err(Error::DecompressFailed);
    }
    // Too big (> 4 MiB)
    if decompressed_size > (1 << 19) * 4 {
        return Err(Error::DecompressFailed);
    }

    let mut out = Vec::with_capacity(decompressed_size);

    while out.len() < decompressed_size {
        let mut flags = cur.next::<u8>()?;
        for _ in 0..8 {
            let compressed = flags & 0x80 != 0;
            flags <<= 1;
            if !compressed {
                // Uncompressed byte
                out.push(cur.next::<u8>()?);
            } else {
                let (ofs, n);

                fn nibbles(x: u8) -> (u8, u8) { (x >> 4, x & 0xf) }
                let (a,b) = nibbles(cur.next::<u8>()?);
                match a {
                    0 => {
                        // ab cd ef
                        // =>
                        // n = abc + 0x11 = bc + 0x11
                        // ofs = def + 1
                        let (c,d) = nibbles(cur.next::<u8>()?);
                        let ef = cur.next::<u8>()?;

                        n = (((b as usize) << 4) | (c as usize)) + 0x11;
                        ofs = (((d as usize) << 8) | ef as usize) + 1;
                    }
                    1 => {
                        // ab cd ef gh
                        // =>
                        // n = bcde + 0x111
                        // ofs = fgh + 1
                        let cd = cur.next::<u8>()?;
                        let (e,f) = nibbles(cur.next::<u8>()?);
                        let gh = cur.next::<u8>()?;

                        n = (((b as usize) << 12) | ((cd as usize) << 4) | (e as usize)) + 0x111;
                        ofs = (((f as usize) << 8) | (gh as usize)) + 1;
                    }
                    _ => {
                        // ab cd
                        // =>
                        // n = a + 1
                        // ofs = bcd + 1
                        let cd = cur.next::<u8>()?;

                        n = (a as usize) + 1;
                        ofs = (((b as usize) << 8) | (cd as usize)) + 1;
                    }
                }

                if out.len() + n > decompressed_size { // too much data
                    return Err(Error::DecompressFailed);
                }
                if out.len() < ofs { // not enough data
                    return Err(Error::DecompressFailed);
                }

                for _ in 0 .. n {
                    let x = out[out.len() - ofs];
                    out.push(x);
                }
            }

            if out.len() >= decompressed_size {
                break;
            }
        }
    }

    Ok(DecompressResult {
        data: out,
        end_cur: cur,
    })
}


type Result<T> = result::Result<T, Error>;

// Don't bother storing any info in the error type.
#[derive(Debug)]
pub enum Error {
    DecompressFailed,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl error::Error for Error {}

impl From<cur::Error> for Error {
    fn from(_: cur::Error) -> Error { Error::DecompressFailed }
}
