use errors::Result;
use std;
use util::bits::BitField;
use util::cur::Cur;

/// The result of successfully calling `de_lz77(buf)`.
pub struct Lz77Result {
    /// The decompressed data.
    pub data: Box<[u8]>,
    /// The position of the end of the LZ77 data in `buf`.
    pub end_pos: usize,
}

/// Decompresses LZ77 data in `buf`.
///
/// LZ77 was a common compression format in GBA/DS games. See
/// http://florian.nouwt.com/wiki/index.php/LZ77_(Compression_Format)
/// for documentation on the format.
pub fn de_lz77(buf: &[u8]) -> Result<Lz77Result> {
    let mut cur = Cur::new(buf);

    let header = cur.next::<u32>()?;
    let ty = header.bits(0,8);
    let decompressed_size = header.bits(8, 32) as usize;

    check!(ty == 0x10)?;

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

            let len = std::cmp::min(len, decompressed_size - outp);

            if outp < ofs + 1 {
                return Err("backreference too large".into())
            }

            for i in 0 .. len {
                out[outp + i] = out[outp - ofs - 1 + i];
            }
            outp += len;
        }
        blocks_compression_method <<= 1;
        num_blocks -= 1;
    }

    Ok(Lz77Result {
        data: out,
        end_pos: cur.pos(),
    })
}
