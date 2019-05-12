//! Build the joint tree from the VertexRecord.
//!
//! Our problem is this: given the symbolic matrix M which is to be applied to
//! each vertex
//!
//!     V(p) = M(p) (vertex pos)
//!
//! (where the dependence is on the pose p, ie. the value of the object
//! matrices) determine a skin which has the same effect. That is, give the
//! folllowing data
//!
//! - a tree of joints
//! - for every joint j, a local-to-parent transform C[j](p), which is an
//!   SMatrix, the totality of which determine the local-to-world transforms
//!   A[j](p) by
//!
//!       A[j](p) = C[root](p) ... C[grandparent j](p) C[parent j](p) C[j](p)
//!
//! - for every vertex, a list of influences (w_i, j_i) where w_i is a number
//!   (the weight) and j_i is a joint
//!
//! such that when the skinning equation is applied
//!
//!     v(p) = ∑_i w_i A[j_i](p) A[j_i](rest)^{-1} V(rest)
//!
//! we have V(p) = v(p) for every pose p.
//!
//! If you had a tree of joints and you wrote down the possible M's you would
//! get you'd find either
//!
//!     (object) (object) ... (object)
//!
//! for vertices that are influenced by a single joint (and (vertex pos) should
//! be in the space of that joint), or
//!
//!     ∑ (weight) (object) (object) ... (object) (inv bind)
//!
//! for vertices influenced by multiple joints (and (vertex pos) should be in
//! its rest model-space position). The vast majority of matrices I've seen do
//! indeed have one of these two forms (and mostly the former).
//!
//! Let's work backwards and see what conditions need to obtain for us to go
//! from matrices to joints. First, an observation:
//!
//! CONSTANT LEAVES ARE SUPERFLUOUS:
//!
//! Suppose we have a solution where there is a leaf joint j with C[j]
//! independant of p. Then A[j](p) = A[parent j](p) C[j] so
//!
//!     A[j](p) A[j](rest)^{-1} =
//!     A[parent j](p) C[j] C[j]^{-1} A[parent j](rest)^{-1} =
//!     A[parent j](p) A[parent j](rest)^{-1}
//!
//! so for the purpose of skinning, we might just as well have used j's parent
//! instead of j in every influence. In other words, constant leaves are
//! superfluous. This manifests below in that we always factor off the longest
//! suffix that is independent of p from a CMatrix before building a joint for
//! it.
//!
//! Now, the solutions:
//!
//! SOLUTION WHEN M IS A (SCALAR TIMES A) CMATRIX:
//!
//! Write M as a scaled composition of SMatrices
//!
//!     M(p) = a M1(p) M2(p) ... Mn(p) K1 K2 ... Km = a M'(p) K
//!
//! where a is a scalar and  K = K1 ... Km is the longest suffix that does not
//! depend on the pose. Then by making a chain of joints j_1 -> ... -> j_n with
//! C[j_i] = M_i we can find a joint j = j_n with A[j] = M'.
//!
//! Then taking j to be the sole influence with a weight of a we get
//!
//!     v(p) = a A[j](p) A[j](rest)^{-1} V(rest)
//!          = a M'(p) M'(rest)^{-1} V(rest)
//!          = a M'(p) M'(rest)^{-1} M'(rest) K (vertex pos)
//!          = a M'(p) K (vertex pos)
//!          = M(p) (vertex pos)
//!          = V(p)
//!
//! SOLUTION WHEN M IS A SKINNING MATRIX:
//!
//! Suppose M(p) = ∑_i a_i M_i(p) K_i where K_i is the longest suffix of each
//! term that does not depend on the pose. Comparing to the skinning formula, we
//! try to interpret this where a_i = w_i, M_i(p) = A[j_i](p), and K_i =
//! A[j_i](rest)^{-1}. Indeed assuming that
//!
//! * (I) M_i(rest) K_i = 1, and
//! * (II) M(rest) = 1, or equivalently, assuming (I), ∑_i a_i = 1
//!
//! we get
//!
//!     v(p) = ∑_i w_i A[j_i](p) A[j_i](rest)^{-1} V(rest)
//!          = ∑_i a_i M_i(p) M_i(rest)^{-1} V(rest)
//!          = ∑_i a_i M_i(p) K_i V(rest)
//!          = ∑_i a_i M_i(p) K_i M(rest) (vertex pos)
//!          = ∑_i a_i M_i(p) K_i (vertex pos)
//!          = M(p) (vertex pos)
//!          = V(p)
//!
//! (There is additionally a trivial solution for M = 0 with an empty influence
//! list.)
//!
//! These are really the same solution for all cases, it is only the conditions
//! under which they apply that differ. The first says that the solution works
//! when M has one term without additional qualifiers and the second says it
//! works when M has >1 term as long as (I) and (II) hold.
//!
//! What should we do with "unusual" matrices that don't fall into any of these
//! cases? These are rare so we just use the same solution again, but we wil
//! give a warning.

use super::vertex_record::VertexRecord;
use super::{AMatrix, SMatrix};
use super::{Influence, Joint, JointTree, NodeIndex, Skeleton, SkinVertex, Transform};
use cgmath::{ApproxEq, Matrix4, One, SquareMatrix};
use nitro::Model;
use petgraph::Direction;

pub fn build_skeleton(vr: &VertexRecord, model: &Model, objects: &[Matrix4<f64>]) -> Skeleton {
    let mut b = Builder::new(model, objects);

    // Caches the right skinvertex for each of the matrices in vr.
    let mut skin_vert_cache: Vec<Option<SkinVertex>> = vec![None; vr.matrices.len()];
    let mut max_num_influences = 0;

    let vertices = vr
        .vertices
        .iter()
        .map(|&mat_idx| {
            if skin_vert_cache[mat_idx as usize].is_none() {
                let mut sv = b.amatrix_to_skinvert(&vr.matrices[mat_idx as usize]);
                simplify_skinvert(&mut sv);
                max_num_influences = max_num_influences.max(sv.influences.len());
                skin_vert_cache[mat_idx as usize] = Some(sv);
            }

            skin_vert_cache[mat_idx as usize].as_ref().unwrap().clone()
        })
        .collect();

    if b.unusual_matrices {
        warn!(
            "unusual matrices encountered in model {}; the skin for this \
             model may function imperfectly",
            model.name
        );
    }

    // IMPORTANT NOTE!! up until this step, the rest_world_to_local field on
    // joints holds the *rest local-to-world*. Only at this step do we invert
    // it. This somewhat confusing use is because it seems slightly better to
    // compute (A B ... C)^{-1} than to compute C^{-1} ... B^{-1} A^{-1}, that
    // is, to do the inverse at the end rather than at each step.
    for joint in b.graph.node_weights_mut() {
        joint.rest_world_to_local = invert_matrix(joint.rest_world_to_local);
    }

    // Bring multiple roots under a universal root if necessary so that the
    // graph becomes a tree.
    if b.roots.len() > 1 {
        b.make_root();
    }
    let root = b.roots[0];

    Skeleton {
        tree: b.graph,
        root,
        vertices,
        max_num_influences,
    }
}

struct Builder<'a, 'b> {
    model: &'a Model,
    objects: &'b [Matrix4<f64>],

    /// The joint graph, which is actually only a forest. We make it a tree at
    /// the end, if necessary (which it usually isn't).
    graph: JointTree,
    /// List of roots of the forest.
    roots: Vec<NodeIndex>,

    /// Set if we encounter any matrices that can't be resolved as skinning
    /// matrices. Used to report an error.
    unusual_matrices: bool,
}

impl<'a, 'b> Builder<'a, 'b> {
    fn new(model: &'a Model, objects: &'b [Matrix4<f64>]) -> Builder<'a, 'b> {
        // Guess capacities
        let graph = JointTree::with_capacity(objects.len(), objects.len());
        let roots = Vec::with_capacity(1);

        let unusual_matrices = false;

        Builder {
            model,
            objects,
            graph,
            roots,
            unusual_matrices,
        }
    }

    /// Add a "universal root", turning the graph into a tree.
    fn make_root(&mut self) {
        if self.roots.len() == 1 {
            match self.graph[self.roots[0]].local_to_parent {
                Transform::Root => {
                    // Already exists.
                    return;
                }
                _ => (),
            }
        }

        let root = self.graph.add_node(Joint {
            local_to_parent: Transform::Root,
            rest_world_to_local: Matrix4::one(),
        });

        for &old_root in &self.roots {
            self.graph.add_edge(root, old_root, ());
        }

        self.roots.clear();
        self.roots.push(root);
    }

    fn amatrix_to_skinvert(&mut self, amatrix: &AMatrix) -> SkinVertex {
        self.detect_unusual_matrices(amatrix);

        let influences = amatrix
            .terms
            .iter()
            .map(|term| {
                let weight = term.weight;
                let joint = self.cmatrix_to_joint(&term.cmat.factors);
                Influence { weight, joint }
            })
            .collect();
        SkinVertex { influences }
    }

    fn cmatrix_to_joint(&mut self, mut factors: &[SMatrix]) -> NodeIndex {
        // Remove the longest suffix of constant SMatrices.
        while let Some((&last, rest)) = factors.split_last() {
            let is_const = match last {
                SMatrix::Object { .. } => false,
                SMatrix::InvBind { .. } => true,
                SMatrix::Uninitialized { .. } => true,
            };
            if !is_const {
                break;
            }
            factors = rest;
        }

        if factors.len() == 0 {
            // This is unlikely.
            // We need a universal root, which has the identity transform.
            self.make_root();
        }

        let mut node = self.find_root(factors[0]);
        for &factor in &factors[1..] {
            node = self.find_child(node, factor)
        }
        node
    }

    /// Find (or create) a root joint whose transform is the given SMatrix.
    fn find_root(&mut self, smat: SMatrix) -> NodeIndex {
        // First, handle when there is a universal root.
        if self.roots.len() == 1 {
            let root = self.roots[0];
            if let Transform::Root = self.graph[root].local_to_parent {
                return self.find_child(root, smat);
            }
        }

        {
            let existing_root =
                self.roots
                    .iter()
                    .find(|&&idx| match self.graph[idx].local_to_parent {
                        Transform::SMatrix(smat2) => smat == smat2,
                        Transform::Root => false,
                    });

            if let Some(&node_idx) = existing_root {
                return node_idx;
            }
        }

        let rest_world_to_local = self.eval_smatrix(smat);
        let new_root = self.graph.add_node(Joint {
            local_to_parent: Transform::SMatrix(smat),
            rest_world_to_local,
        });

        self.roots.push(new_root);

        new_root
    }

    /// Find (or create) a child of the given node whose transform is the given
    /// SMatrix.
    fn find_child(&mut self, node: NodeIndex, smat: SMatrix) -> NodeIndex {
        let existing_child = self
            .graph
            .neighbors_directed(node, Direction::Outgoing)
            .find(|&idx| match self.graph[idx].local_to_parent {
                Transform::SMatrix(smat2) => smat == smat2,
                Transform::Root => false,
            });

        if let Some(node_idx) = existing_child {
            return node_idx;
        }

        let rest_world_to_local = self.graph[node].rest_world_to_local * self.eval_smatrix(smat);
        let new_child = self.graph.add_node(Joint {
            local_to_parent: Transform::SMatrix(smat),
            rest_world_to_local,
        });

        self.graph.add_edge(node, new_child, ());
        new_child
    }

    fn detect_unusual_matrices(&mut self, amatrix: &AMatrix) {
        // Use fairly generous epsilons here.

        if amatrix.terms.len() > 1 {
            let mut sum = 0.0;
            for term in &amatrix.terms {
                sum += term.weight;

                // Check (I)
                let rest_cmat = self.eval_cmatrix(&term.cmat.factors);
                if rest_cmat.relative_ne(&Matrix4::one(), 0.1, 0.1) {
                    self.unusual_matrices = true;
                }
            }

            // Check (II)
            if (sum - 1.0).abs() > 0.1 {
                self.unusual_matrices = true;
            }
        }
    }

    /// Evaluates an SMatrix in the rest pose.
    fn eval_smatrix(&self, smat: SMatrix) -> Matrix4<f64> {
        match smat {
            SMatrix::Object { object_idx } => self.objects[object_idx as usize],
            SMatrix::InvBind { inv_bind_idx } => self.model.inv_binds[inv_bind_idx as usize],
            SMatrix::Uninitialized { .. } => Matrix4::one(),
        }
    }

    /// Evaluates a CMatrix in the rest pose.
    fn eval_cmatrix(&self, factors: &[SMatrix]) -> Matrix4<f64> {
        factors
            .iter()
            .map(|&smat| self.eval_smatrix(smat))
            .product()
    }
}

fn simplify_skinvert(sv: &mut SkinVertex) {
    // Group like terms.
    for i in 0..sv.influences.len() {
        for j in (i + 1)..sv.influences.len() {
            if sv.influences[i].joint == sv.influences[j].joint {
                sv.influences[i].weight += sv.influences[j].weight;
                sv.influences[j].weight = 0.0;
            }
        }
    }
    sv.influences.retain(|influence| influence.weight != 0.0);

    // Sort by weight, highest to lowest
    sv.influences
        .sort_by(|in1, in2| in2.weight.partial_cmp(&in1.weight).unwrap());
}

/// Inverts a matrix, bumping its entries slightly if necessary until it is
/// non-singular. Assumes the final row is (0 0 0 1).
fn invert_matrix(mut mat: Matrix4<f64>) -> Matrix4<f64> {
    let mut rng = 0x83e17875_u32;
    loop {
        if let Some(inv) = mat.invert() {
            return inv;
        }

        // Apply random bumps to the upper-left 3x3 subblock in the hope the
        // matrix moves off the variety det(m) = 0.
        let a = rng as usize;
        // NOTE: the smallest representable number on the DS is ~0.0002
        static EPS: [f64; 5] = [0.000012, -0.000017, 0.000006, -0.000008, 0.00001];
        mat[0][(a + 0) % 3] += EPS[(a + 0) % EPS.len()];
        mat[1][(a + 1) % 3] += EPS[(a + 1) % EPS.len()];
        mat[2][(a + 2) % 3] += EPS[(a + 2) % EPS.len()];

        rng ^= rng << 17;
        rng ^= rng >> 13;
        rng ^= rng << 5; // xorshift
    }
}
