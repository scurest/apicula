//! Symbolic matrices and their algebra.

use std::ops;

/// The simplest possible symbolic matrix, out of which the others are built.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum SMatrix {
    /// An object matrix from the model file. Their value depends on the pose
    /// (and in fact a pose is exactly the set of values of the object matrices;
    /// everything else is static).
    Object { object_idx: u8 },
    /// An inverse bind matrix from the model file.
    InvBind { inv_bind_idx: u8 },
    /// The contents of an uninitialized matrix slot, ie. one that we haven't
    /// stored to yet. Theoretically possible, but shouldn't show up.
    Uninitialized { stack_pos: u8 },
}

/// Composition of SMatrices.
#[derive(Clone)]
pub struct CMatrix {
    pub factors: Vec<SMatrix>,
}

/// Linear combination of CMatrices.
#[derive(Clone)]
pub struct AMatrix {
    pub terms: Vec<ATerm>,
}

#[derive(Clone)]
pub struct ATerm {
    pub weight: f32,
    pub cmat: CMatrix,
}

// SMatrix -> CMatrix
impl From<SMatrix> for CMatrix {
    fn from(smat: SMatrix) -> CMatrix {
        CMatrix {
            factors: vec![smat],
        }
    }
}

// CMatrix -> AMatrix
impl From<CMatrix> for AMatrix {
    fn from(cmat: CMatrix) -> AMatrix {
        AMatrix {
            terms: vec![ATerm { weight: 1.0, cmat }],
        }
    }
}

// SMatrix -> AMatrix
impl From<SMatrix> for AMatrix {
    fn from(smat: SMatrix) -> AMatrix {
        let cmat: CMatrix = smat.into();
        cmat.into()
    }
}

impl CMatrix {
    /// Identity CMatrix
    pub fn one() -> CMatrix {
        CMatrix { factors: vec![] }
    }
}

impl AMatrix {
    /// Identity AMatrix
    pub fn one() -> AMatrix {
        CMatrix::one().into()
    }
    /// Zero AMatrix
    pub fn zero() -> AMatrix {
        AMatrix { terms: vec![] }
    }
}

// AMatrix *= SMatrix
impl<'a> ops::MulAssign<SMatrix> for AMatrix {
    fn mul_assign(&mut self, smat: SMatrix) {
        // Distribute it over the sum
        for term in &mut self.terms {
            term.cmat.factors.push(smat);
        }
    }
}

// AMatrix *= f32
impl<'a> ops::MulAssign<f32> for AMatrix {
    fn mul_assign(&mut self, t: f32) {
        if t == 0.0 {
            self.terms.clear();
            return;
        }
        // Distribute it over the sum
        for term in &mut self.terms {
            term.weight *= t;
        }
    }
}

// AMatrix += AMatrix
impl<'a> ops::AddAssign<AMatrix> for AMatrix {
    fn add_assign(&mut self, amat: AMatrix) {
        // Don't group like terms; we'll do that after making them into
        // SkinVertices since it's easier to compare NodeIndices than CMatrices.
        // Note that it is impossible for terms to cancel out, since there is no
        // way to encode a negative scalar in the skinning rendering command.
        self.terms.extend(amat.terms)
    }
}
