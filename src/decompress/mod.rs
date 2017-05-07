use errors::Result;
use errors::ResultExt;
use util::bits::BitField;
use util::cur::Cur;

/// The result of successfully calling `try_decompress(cur)`.
pub struct DecompressResult<'a> {
    /// The decompressed data.
    pub data: Box<[u8]>,
    /// A cursor at the end of the compressed data stream.
    pub end_cur: Cur<'a>,
}

/// Try to decompress data around `cur`.
///
/// The compression method is guessed from the methods that were
/// included in the GBA/DS BIOS. See
/// http://problemkaputt.de/gbatek.htm#biosdecompressionfunctions.
///
/// Presently, only LZ77 is tried (an attempt at RLE did not bear
/// fruit).
pub fn try_decompress(cur: Cur) -> Result<DecompressResult> {
    try_decompress_lz77(cur)
}

fn try_decompress_lz77(mut cur: Cur) -> Result<DecompressResult> {
    // Assume `cur` is at the first byte of the stream's data.
    // That is
    //
    //     v-- header starts here
    //     10 XX XX XX 0X [STAMP]
    //                    ^-- `cur` is here
    //
    // So back up by 5 and then decompress LZ77 data.
    if cur.pos() < 5 {
       bail!("not enough space for LZ77 header");
    }
    let pos = cur.pos();
    cur.jump_to(pos - 5).unwrap();

    de_lz77(cur)
}

fn de_lz77(mut cur: Cur) -> Result<DecompressResult> {
    let header = cur.next::<u32>()?;
    let ty = header.bits(4,8);
    let decompressed_size = header.bits(8, 32) as usize;

    check!(ty == 1)
        .chain_err(|| "compression method wasn't LZ77")?;

    let mut out = vec![0; decompressed_size].into_boxed_slice();
    let mut outp = 0;

    let mut num_blocks = 0;
    let mut blocks_compression_method = 0;

    while outp < decompressed_size {
        if num_blocks == 0 {
            blocks_compression_method = cur.next::<u8>()?;
            num_blocks = 8;
        }

        let compression_method = blocks_compression_method.bits(7,8);
        if compression_method == 0 {
            out[outp] = cur.next::<u8>()?;
            outp += 1;
        } else {
            let data = cur.next::<u16>()?.swap_bytes(); // stored in big-endian
            let ofs = data.bits(0, 12) as usize;
            let n = data.bits(12, 16) as usize;

            let len = n + 3;

            check!(outp + len <= decompressed_size)
                .chain_err(|| "backreference copies too much data")?;
            check!(outp >= ofs + 1)
                .chain_err(|| "backreference to OOB data")?;

            for i in 0 .. len {
                out[outp + i] = out[outp - ofs - 1 + i];
            }
            outp += len;
        }

        blocks_compression_method <<= 1;
        num_blocks -= 1;
    }

    Ok(DecompressResult {
        data: out,
        end_cur: cur,
    })
}
