//! Decodes formats for 3x3 matrices (usually rotations).

use cgmath::Matrix;
use cgmath::Matrix3;
use cgmath::Matrix4;
use cgmath::vec3;
use cgmath::Vector4;
use errors::Result;
use util::bits::BitField;
use util::fixed::fix16;

pub fn pivot_mat(select: u16, neg: u16, a: f64, b: f64) -> Result<Matrix4<f64>> {
    if select > 9 {
        bail!("unknown pivot select: {}", select);
    }

    if select == 9 {
        // Does this actually happen?
        return Ok(Matrix4::new(
            -a,  0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ));
    }

    let o = if neg.bits(0,1) == 0 { 1.0 } else { -1.0 };
    let c = if neg.bits(1,2) == 0 { b } else { -b };
    let d = if neg.bits(2,3) == 0 { a } else { -a };

    // `select` chooses the result in the following way
    //
    //     o..    .o.    ..o
    //   0=.ac  3=a.c  6=ac.
    //     .bd    b.d    bd.
    //
    //     .ac    a.c    ac.
    //   1=o..  4=.o.  7=..o
    //     .bd    b.d    bd.
    //
    //     .ac    a.c    ac.
    //   2=.bd  5=b.d  8=bd.
    //     o..    .o.    ..o
    //
    // Note that they are all permutations of the rows
    // and columns of the first matrix.
    let mat = Matrix4::new(
         o,  0.0, 0.0, 0.0,
        0.0,  a,   b,  0.0,
        0.0,  c,   d,  0.0,
        0.0, 0.0, 0.0, 1.0,
    );
    let pi_cols = match select / 3 {
        0 => pi(1,2,3,4),
        1 => pi(2,1,3,4),
        2 => pi(2,3,1,4),
        _ => unreachable!(),
    };
    let pi_rows = match select % 3 {
        0 => pi(1,2,3,4),
        1 => pi(2,1,3,4),
        2 => pi(2,3,1,4),
        _ => unreachable!(),
    };
    Ok(pi_rows.transpose() * mat * pi_cols)
}

/// The permutation matrix corresponding to the permutation of {1,2,3,4} sending
/// 1 to a, 2 to b, etc.
///
/// The matrix is the image of the permutation regarded as a map {1,2,3,4} -> {1,2,3,4}
/// under the free vector space functor. That is, it is the identity matrix with the
/// columns permuted according to the given permutation.
///
/// The upshot is m * pi(...) permutes the columns of m and pi(...)^T * m permutes
/// the rows.
fn pi(a: usize, b: usize, c: usize, d: usize) -> Matrix4<f64> {
    assert_eq!(
        [1,2,3,4],
        { let mut arr = [a,b,c,d]; arr.sort(); arr }
    );
    let basis = [
        Vector4::unit_x(),
        Vector4::unit_y(),
        Vector4::unit_z(),
        Vector4::unit_w(),
    ];
    Matrix4::from_cols(basis[a-1], basis[b-1], basis[c-1], basis[d-1])
}

pub fn basis_mat((in0,in1,in2,in3,in4): (u16,u16,u16,u16,u16)) -> Matrix4<f64> {
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
    // find a good way to locate it with the debugger.

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

    Matrix3::from_cols(a, b, c).into()
}
