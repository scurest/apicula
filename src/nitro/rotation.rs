//! Decodes formats for rotations.
//!
//! Rotations are always stored as a 3x3 matrix (probably for speed; the NDS
//! probably wasn't fast enough to convert Euler angles or quaternions to
//! matrices on the fly). This also means that a "rotation" matrix might not
//! actually be a rotation (ie. orthogonal of determinant +1).

use cgmath::{Matrix3, vec3};
use util::bits::BitField;
use util::fixed::fix16;

pub fn pivot_mat(select: u16, neg: u16, a: f64, b: f64) -> Matrix3<f64> {
    if select >= 9 {
        // Does this actually happen?
        debug!("pivot with select={} actually happened! :O", select);
        return Matrix3::new(
            -a,  0.0, 0.0,
            0.0, 0.0, 0.0,
            0.0, 0.0, 0.0,
        );
    }

    let o = if neg.bits(0,1) == 0 { 1.0 } else { -1.0 };
    let c = if neg.bits(1,2) == 0 { b } else { -b };
    let d = if neg.bits(2,3) == 0 { a } else { -a };

    // Consider eg. a = cos θ, b = sin θ.
    // Nb. the pattern here.
    match select {
        0 => Matrix3::new( o , 0.0, 0.0,  0.0,  a ,  b ,  0.0,  c ,  d ),
        1 => Matrix3::new(0.0,  o , 0.0,   a , 0.0,  b ,   c , 0.0,  d ),
        2 => Matrix3::new(0.0, 0.0,  o ,   a ,  b , 0.0,   c ,  d , 0.0),

        3 => Matrix3::new(0.0,  a ,  b ,   o , 0.0, 0.0,  0.0,  c ,  d ),
        4 => Matrix3::new( a , 0.0,  b ,  0.0,  o , 0.0,   c , 0.0,  d ),
        5 => Matrix3::new( a ,  b , 0.0,  0.0, 0.0,  o ,   c ,  d , 0.0),

        6 => Matrix3::new(0.0,  a ,  b ,  0.0,  c ,  d ,   o , 0.0, 0.0),
        7 => Matrix3::new( a , 0.0,  b ,   c , 0.0,  d ,  0.0,  o , 0.0),
        8 => Matrix3::new( a ,  b , 0.0,   c ,  d , 0.0,  0.0, 0.0,  o ),

        _ => unreachable!(),
    }
}

pub fn basis_mat((in0,in1,in2,in3,in4): (u16,u16,u16,u16,u16)) -> Matrix3<f64> {
    // Credit for figuring this out goes to MKDS Course Modifier.
    //
    // Braindump for this function follows:
    //
    // The matrix is specified by giving an orthonormal right-hand basis to transform
    // to. This is 9 numbers (3 entries for 3 vectors). Since the basis is orthonormal
    // and right-handed, 3 of these are redundant (given by the cross-product), so we
    // only need to store 6 numbers. Since the vectors are unit length, the components
    // are all <1 in magnitude (=+1 does not appear to be possible?) and the DS uses
    // 12 bits for the fractional part of its numbers, so we need 13 bits (+1 for sign)
    // for each number. 13 * 6 = 78 bits. Give one of the numbers an extra 2 bits, and
    // we get exactly five u16s.
    //
    // The obvious way to store each 14-bit number is packed densely one after the other.
    // But extracting them this way requires a different mask/shift/accumulate for every
    // one. Instead, five of the numbers are stored in the high 13 bits of each u16 and
    // the last number is built by xoring the low three bits together in sequence. This
    // can be done in a simple loop.
    //
    // in4 is handled strangely. The first three entries come from (in0,in1,in2), but
    // the last entry, is made from a sequence of xors
    //   (in4 << 12) | (in0 << 9) | (in1 << 6) | (in2 << 3) | in3
    // which is where the weird permutation 4,0,1,2,3 of the input comes from. I don't
    // have a good explanation for this part :(
    //
    // I'd like to check this code against the disassembly from a DS ROM, but I can't
    // find a way to locate it with the debugger.

    let input = [in4, in0, in1, in2, in3];
    let mut out = [0u16; 6];

    for i in 0..5 {
        out[i] = input[i].bits(3,16);
        out[5] = (out[5] << 3) | input[i].bits(0,3);
    }

    let f = |x| fix16(x, 1, 0, 12);
    let a = vec3(f(out[1]), f(out[2]), f(out[3]));
    let b = vec3(f(out[4]), f(out[0]), f(out[5]));
    let c = a.cross(b);

    Matrix3::from_cols(a, b, c)
}
