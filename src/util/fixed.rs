//! Fixed-point to `f64` conversions.

use util::bits::BitField;

/// Reads a fixed-point number from a `u32`.
///
/// A fixed-point number represents a fractional value (eg. 5.3) as an integer
/// in smaller units (eg. 53 tenths). Ie. in contrast to floating-point numbers,
/// there are a fixed number of bits after the decimal point. The format
/// (`sign_bits`,`int_bits`,`frac_bits`) determines how many bits are in the
/// fractional part, the integer part, and whether the number is signed.
///
/// Precisely, the low `sign_bits + int_bits + frac_bits` bits of `x` are interpreted
/// as an integer (unsigned if `sing_bits` is 0, twos-complement if it is 1),
/// and the result is this integer times 2^(-`frac_bits`).
pub fn fix32(x: u32, sign_bits: u32, int_bits: u32, frac_bits: u32) -> f64 {
    assert!(sign_bits <= 1);
    assert!(int_bits + frac_bits > 0);
    assert!(sign_bits + int_bits + frac_bits <= 32);

    let x = x.bits(0, sign_bits + int_bits + frac_bits);

    let y = if sign_bits == 0 {
        x as f64
    } else {
        // sign extend
        let sign_mask = (1 << (int_bits + frac_bits)) as u32;
        if x & sign_mask != 0 {
            (x | !(sign_mask - 1)) as i32 as f64
        } else {
            x as f64
        }
    };
    y * 0.5f64.powi(frac_bits as i32)
}

/// Like `fix32` but for `u16`s. For convenience.
pub fn fix16(x: u16, sign_bits: u32, int_bits: u32, frac_bits: u32) -> f64 {
    assert!(sign_bits + int_bits + frac_bits <= 16);
    fix32(x as u32, sign_bits, int_bits, frac_bits)
}
