use cgmath::Matrix;
use cgmath::Matrix4;
use cgmath::vec3;
use cgmath::Vector4;
use errors::Result;
use util::bits::BitField;
use util::cur::Cur;

pub fn read_translation(cur: &mut Cur) -> Result<Matrix4<f64>> {
    fields!(*cur, tranlation {
        x: (fix32(1,19,12)),
        y: (fix32(1,19,12)),
        z: (fix32(1,19,12)),
        end: Cur,
    });
    *cur = end;
    Ok(Matrix4::from_translation(vec3(x, y, z)))
}

pub fn read_rotation(cur: &mut Cur, flags: u16) -> Result<Matrix4<f64>> {
    fields!(*cur, rot {
        a: (fix16(1,3,12)),
        b: (fix16(1,3,12)),
        end: Cur,
    });
    *cur = end;
    let select = flags.bits(4,8);
    let neg = flags.bits(8,12);
    pivot_mat(select, neg, a, b)
}

pub fn read_matrix(cur: &mut Cur, m0: f64) -> Result<Matrix4<f64>> {
    fields!(*cur, rot {
        m1: (fix16(1,3,12)),
        m2: (fix16(1,3,12)),
        m3: (fix16(1,3,12)),
        m4: (fix16(1,3,12)),
        m5: (fix16(1,3,12)),
        m6: (fix16(1,3,12)),
        m7: (fix16(1,3,12)),
        m8: (fix16(1,3,12)),
        end: Cur,
    });
    *cur = end;
    Ok(Matrix4::new(
        m0,  m1,  m2,  0.0,
        m3,  m4,  m5,  0.0,
        m6,  m7,  m8,  0.0,
        0.0, 0.0, 0.0, 1.0,
    ))
}

pub fn read_scale(cur: &mut Cur) -> Result<Matrix4<f64>> {
    fields!(*cur, scale {
        sx: (fix32(1,19,12)),
        sy: (fix32(1,19,12)),
        sz: (fix32(1,19,12)),
        end: Cur,
    });
    *cur = end;
    Ok(Matrix4::from_nonuniform_scale(sx,sy,sz))
}

pub fn pivot_mat(select: u16, neg: u16, a: f64, b: f64) -> Result<Matrix4<f64>> {
    if select > 9 {
        return Err(format!("unknown pivot select: {}", select).into());
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
