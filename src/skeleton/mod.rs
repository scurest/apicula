//! Build a skeleton for a model.
//!
//! A brief review of how skinning works. The skeleton consists of a _tree of
//! joints_ and a _skin vertex_ for every vertex.
//!
//! Each joint in the tree has a local-to-parent transform. The composition of
//! the local-to-parent of a joint with its parent's local-to-parent transform,
//! and so on all the way up the tree is a the joint's local-to-world transform.
//! Example:
//!
//!      A       A's local-to-parent = a, B's = b, etc.
//!     / \      D's local-to-world = a c d
//!    B   C     D's world-to-local = d^{-1} c^{-1} a^{-1}
//!       /
//!      D
//!
//! A skin vertex consists of a list of influences, each influence containing a
//! joint that influences the vertex and a weight controlling how great the
//! influence is. The vertex's final position is determined by the skinning
//! equation
//!
//!     (vertex final pos) =
//!       ∑_{influence}
//!         (weight) (pose local-to-world) (rest world-to-local) (vertex rest pos)
//!
//! See eg. section X in the COLLADA 1.4 spec for more details.
//!
//! In NSBMD models, this skeleton data has been compiled into an imperative
//! list of rendering commands. Example for the above tree:
//!
//!     multiply cur matrix by a
//!     multiply cur matrix by c
//!     multiply cur matrix by d
//!     store to stack slot 1
//!     ... later ...
//!     restore stack slot 1
//!     draw vertices for D
//!     etc.
//!
//! Our job here is to essentially reverse this procedure, reconstructing  the
//! skeleton data from this list of imperative commands.
//!
//! We do this by doing abstract interpretation of the rendering commands,
//! recording what symbolic matrix is in which slot at each point in time. So we
//! would know that slot 1 contained a c d, and we would then use that to
//! reconstruct a chain of joints A -> C -> D and use D as the joint for
//! vertices drawn after restoring stack slot 1.
//!
//! This is the simplest case. A more detailed analysis of the math is in
//! joint_tree.rs.

pub mod symbolic_matrix;
mod vertex_record;
mod joint_tree;

pub use self::symbolic_matrix::{SMatrix, CMatrix, AMatrix, ATerm};

use cgmath::Matrix4;
use nitro::Model;
use util::tree::{Tree, NodeIdx};

/// Skeleton (or skin) for a model.
pub struct Skeleton {
    pub tree: Tree<Joint>,
    pub root: NodeIdx,
    pub max_num_weights: u8, // max weights on any vertex

    pub weights: Vec<Weight>, // weights for all verts packed together
    verts: Vec<WeightsOfs>, // verts[vi] points to the weights for vertex vi in weights
}

pub struct Joint {
    pub local_to_parent: Transform,
    // Ie. inverse bind matrix. Cached for convenience.
    pub rest_world_to_local: Matrix4<f64>,
}

// A joint's local-to-parent transform.
pub enum Transform {
    /// Same value as an SMatrix (usually an object matrix).
    SMatrix(SMatrix),
    /// If the graph of joints is a forest and not a tree, it may be necessary
    /// to insert a node to turn it into a tree. That node (called the universal
    /// root) will have this transform. Treat as the identity.
    Root,
}

#[derive(Copy, Clone)]
pub struct Weight {
    pub weight: f32,
    pub joint: NodeIdx,
}

/// Points to a slice skel.weights[start..start+len].
/// start is the low 24 bits, len is the high 8 bits.
#[derive(Copy, Clone)]
struct WeightsOfs(u32);


impl Skeleton {
    pub fn vert_weights(&self, vi: usize) -> &[Weight] {
        let ofs = self.verts[vi];
        let start = (ofs.0 & 0xffffff) as usize;
        let len = (ofs.0 >> 24) as usize;
        &self.weights[start .. start + len]
    }

    pub fn build(model: &Model, objects: &[Matrix4<f64>]) -> Skeleton {
        // First play back rendering commands recording the symbolic value of
        // the matrix applied to every vertex.
        let vr = self::vertex_record::VertexRecord::build_for_model(model);
        // Then build a joint tree for those matrices.
        let skel = self::joint_tree::build_skeleton(&vr, model, objects);
        skel
    }
}
