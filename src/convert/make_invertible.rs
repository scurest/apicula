use cgmath::Matrix4;

/// Slightly perturb a matrix's diagonal to create a non-singular matrix.
///
/// For example, if an object matrix is zero (to eg. hide a part of a mesh
/// for some portion of an animation) this will make a new object matrix
/// that "hides" it by making it very small.
pub fn make_invertible(m: &Matrix4<f64>) -> Matrix4<f64> {
    use cgmath::{SquareMatrix, One};

    if m.is_invertible() {
        return m.clone();
    }

    // Try making the matrix invertible by bumping it slightly along the
    // diagonal.
    for &epsilon in &[0.000001, 0.00001, 0.0001, 0.001f64] {
        let m2 = m + Matrix4::from_scale(epsilon);
        if m2.is_invertible() {
            return m2;
        }
    }

    // Fuck this, I give up.
    warn!("found singular object matrix (COLLADA requires an invertible \
        matrix here); proceeding with the identity. Your model may look wrong.");
    debug!("namely, the matrix {:#?}", m);
    Matrix4::one()
}
