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
///
/// The compression method is guessed from the methods that were included in the
/// GBA/DS BIOS. See
/// <http://problemkaputt.de/gbatek.htm#biosdecompressionfunctions>.
pub fn decompress(cur: Cur) -> Result<DecompressResult> {
    match cur.peek::<u8>() {
        Ok(0x10) => de_lz77_0x10(cur),
        //Ok(0x11) => de_lz77_0x11(cur),
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

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::DecompressFailed => "decompress failed",
        }
    }
}

impl From<cur::Error> for Error {
    fn from(_: cur::Error) -> Error { Error::DecompressFailed }
}
