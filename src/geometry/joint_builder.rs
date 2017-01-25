//! Build the joint tree.
//!
//! Models are drawn on the DS with a series of imperative commands,
//! eg. multiply by this matrix, store to the stack, draw this vertex,
//! multiply by this other matrix, store to the stack, etc.
//!
//! COLLADA on the other hand uses an immutable tree of joint nodes. Each
//! node gives the transform from its own space to its parent space, and
//! the transform from its own space to world space is given by the
//! concatenation of all the transforms of its ancestors. Each vertex is
//! then skinned by multiplying it with a weighted combination of these joint
//! matrices.
//!
//! To correct this impedence mismatch, this module performs abstract
//! interpretation of the render commands for the model to build the data
//! structures for COLLADA.
//!
//! --------
//!
//! First, a brief survey of the render commands:
//!
//! 1. initially, the current matrix is the identity and the matrix stack
//!    contains unknown data
//! 2. the current matrix can be premultiplied by an object matrix
//! 3. a matrix on the stack can be loaded into the current matrix
//! 4. the current matrix can be stored to a location on the stack
//! 5. the current matrix can be set to a weighted combination of products
//!    of the form (stack matrix) * (blend_matrix)
//!
//! The current matrix is therefore always a linear combination of matrix
//! products of object matrices, blend matrices, the identity, and possibly
//! unknown data from the initial stack.
//!
//! A COLLADA vertex's position is happily also a linear combination of
//! joint matrices. We therefore distinguish two kinds of matrices. First,
//! a matrix that is a straight composition of object, blend, etc. matrices.
//! These will be become a node in a tree of joints. And second, a linear
//! combination of these node matrices. These will become weights in the
//! <vertex_weights> element.
//!
//! The first kind of matrices are represented as indices into a graph of
//! nodes. The tree structure is just determined by the structure of
//! multiplication (eg. premultiplication <=> parent-child relationship).
//! The second kind is represented as a formal linear combination of these
//! indices which we manipulate algebarically. Every matrix used by the GPU
//! is one of these linear combinations.

use cgmath::Matrix4;
use cgmath::One;
use cgmath::SquareMatrix;
use nitro::mdl::Model;
use petgraph::Direction;
use petgraph::stable_graph::StableGraph;
use petgraph::graph::NodeIndex;

/// Tree of joints. The convention for edges is that they run _from_
/// the parent _to_ the child.
///
/// See `JointBuilder::data` for the reason we use a `StableGraph`.
pub type JointTree = StableGraph<Node, ()>;

#[derive(Debug, Clone)]
pub struct Node {
    pub transform: Transform,
    /// The world-to-local transform at this node. It could be computed
    /// by walking up the tree and multiplying inverses of the transforms,
    /// but we cache it here for convenience.
    pub inv_bind_matrix: Matrix4<f64>,
}

/// Represents the local-to-parent transform of a joint in the tree.
/// There is one variant for every kind of matrix that can be multiplied
/// by.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Transform {
    Root,
    Object(u8),
    /// A "dummy" node representing the concatenation of a matrix with
    /// one the static blend matrices. These nodes are never animated, btw.
    BlendDummy(u8),
    /// The value in the given stack slot when we began rendering. Since
    /// we haven't written a known matrix to it, we don't know what value
    /// it holds (we treat it as the identity). This should generally not
    /// show up.
    UnknownStackSlot(u8),
}

/// A linear combination of matrices from the joint tree,
/// represented as Σ (weight * local-to-world matrix for node).
///
/// TODO: the great majority of these are a single term and
/// I have never seen more than three terms. Use a SmallVec?
#[derive(Debug, Clone, PartialEq)]
pub struct LinComb(pub Vec<LinCombTerm>);
#[derive(Debug, Clone, PartialEq)]
pub struct LinCombTerm {
    pub weight: f64,
    pub joint_id: NodeIndex,
}

#[derive(Debug, Clone)]
pub struct JointBuilder<'a, 'b: 'a> {
    data: JointData,
    model: &'a Model<'b>,
    cur_matrix: LinComb,
    matrix_stack: Vec<Option<LinComb>>,
}

#[derive(Debug, Clone)]
pub struct JointData {
    pub tree: JointTree,
    pub root: NodeIndex,
    /// The nth entry is the matrix which should be applied to the nth vertex.
    pub vertices: Vec<LinComb>,
}

impl<'a, 'b: 'a> JointBuilder<'a, 'b> {
    pub fn new(model: &'a Model<'b>) -> JointBuilder<'a, 'b> {
        let mut tree = StableGraph::new();
        let root = tree.add_node(Node {
            transform: Transform::Root,
            inv_bind_matrix: Matrix4::one(),
        });
        JointBuilder {
            data: JointData {
                tree: tree,
                root: root,
                vertices: vec![],
            },
            model: model,
            cur_matrix: LinComb::from_joint(root),
            matrix_stack: vec![None; 32],
        }
    }
    pub fn data(self) -> JointData {
        let mut data = self.data;

        // If there is a single child of the dummy root node, delete
        // out dummy and use its single child as the new root. This
        // results in a slightly cleaner skeleton. This is only reason
        // we need a `StableGraph` instead of a `Graph`.
        //
        // NOTE: it is very unlikely, but possible, that a vertex actually
        // _uses_ the dummy root. Check for this.
        let root = data.root;
        let root_child = get_single(
            data.tree.neighbors_directed(root, Direction::Outgoing)
        );
        if let Some(r) = root_child {
            data.tree.remove_node(root);
            data.root = r;
        }

        data
    }
    pub fn load_matrix(&mut self, stack_pos: u8) {
        self.cur_matrix = self.get_from_stack(stack_pos);
    }
    pub fn mul_by_object(&mut self, object_id: u8) {
        let cur_matrix = self.cur_matrix.clone();
        let new = self.mul_comb(cur_matrix, Transform::Object(object_id));
        self.cur_matrix = new;
    }
    pub fn store_matrix(&mut self, stack_pos: u8) {
        self.matrix_stack[stack_pos as usize] = Some(self.cur_matrix.clone());
    }
    pub fn blend(&mut self, stack_pos: u8, terms: &[((u8, u8), f64)]) {
        // Set the current matrix to Σ (weight * stack_matrix * blend_matrix)
        // and store it to the stack. IME, these never compose (ie. stack_matrix
        // is never a non-trivial linear combination) but we still handle that
        // case by distributing and grouping like terms.
        let mut distributed = LinComb(terms.iter()
            .flat_map(|&((stack_id, blend_id), weight)| {
                let stack = self.get_from_stack(stack_id);
                let mut stack_times_blend = self.mul_comb(stack, Transform::BlendDummy(blend_id));
                stack_times_blend.mul_scalar_in_place(weight); // distribute scalar
                stack_times_blend.0
            })
            .collect::<Vec<_>>()
        );
        distributed.group_like_terms_in_place();
        self.cur_matrix = distributed;
        self.store_matrix(stack_pos);
    }
    pub fn vertex(&mut self) {
        self.data.vertices.push(self.cur_matrix.clone());
    }
    /// Premultiplies a linear combination by the given transform.
    fn mul_comb(&mut self, v: LinComb, xform: Transform) -> LinComb {
        // v * xform = (Σ weight * node) = Σ weight * (node * xform)
        LinComb(v.0.iter().
            map(|&LinCombTerm { weight, joint_id }| {
                LinCombTerm { weight: weight, joint_id: self.find_or_add_child(joint_id, xform) }
            })
            .collect()
        )
    }
    /// Returns the matrix at `stack_pos`. Handles unknown stack slots
    /// for you.
    fn get_from_stack(&mut self, stack_pos: u8) -> LinComb {
        self.matrix_stack[stack_pos as usize].clone()
            .unwrap_or_else(|| {
                let root = self.data.root;
                let node_idx = self.find_or_add_child(
                    root,
                    Transform::UnknownStackSlot(stack_pos),
                );
                LinComb::from_joint(node_idx)
            })
    }
    /// Returns a child of the node `at` with the given transform value. If one exists
    /// (there should be at most one), it will be used; otherwise, one is created.
    ///
    /// This corresponds to multiplying the matrix reprented by `at` by the given transform.
    fn find_or_add_child(&mut self, at: NodeIndex, transform: Transform) -> NodeIndex {
        let found = self.data.tree
            .neighbors_directed(at, Direction::Outgoing)
            .find(|&idx| self.data.tree[idx].transform == transform);
        match found {
            Some(idx) => idx,
            None => {
                let parent_inv_bind = self.data.tree[at].inv_bind_matrix;
                let object_mat = self.transform_to_matrix(transform);
                let inv_object_mat = object_mat.invert()
                    .unwrap_or_else(|| {
                        warn!("while building inverse bind matrix, a non-\
                            invertible matrix was encountered");
                        Matrix4::one() // try to keep going...
                    });
                let new_child = self.data.tree.add_node(Node {
                    transform: transform,
                    inv_bind_matrix: inv_object_mat * parent_inv_bind,
                });
                self.data.tree.add_edge(at, new_child, ());
                new_child
            }
        }
    }
    /// Gets the local-to-parent matrix for the given transform.
    fn transform_to_matrix(&self, transform: Transform) -> Matrix4<f64> {
        match transform {
            Transform::Root => Matrix4::one(),
            Transform::Object(id) => self.model.objects[id as usize].xform,
            Transform::BlendDummy(id) => self.model.blend_matrices[id as usize].0,
            Transform::UnknownStackSlot(_) => Matrix4::one(),
        }
    }
}

/// If `iter` yields exactly one element, returns that element. Otherwise,
/// returns `None`.
fn get_single<I: Iterator>(mut iter: I) -> Option<<I as Iterator>::Item> {
    let first = iter.next();
    if let Some(x) = first {
        if iter.next().is_none() {
            return Some(x);
        }
    }
    None
}

impl LinComb {
    /// Returns a linear combination consisting only of the given
    /// node matrix (monadic `pure`).
    fn from_joint(joint_id: NodeIndex) -> LinComb {
        LinComb(vec![LinCombTerm { weight: 1.0, joint_id: joint_id }])
    }

    fn mul_scalar_in_place(&mut self, lam: f64) {
        for term in &mut self.0 {
            term.weight *= lam;
        }
    }

    fn group_like_terms_in_place(&mut self) {
        // For every term t, move the factor from any subsequent like
        // term into the factor for t. Then remove all terms with factor
        // zero. Don't worry about this being slow, since these vectors
        // are tiny.
        for i in 0..self.0.len() {
            for j in i+1..self.0.len() {
                if self.0[i].joint_id == self.0[j].joint_id {
                    self.0[i].weight += self.0[j].weight;
                    self.0[j].weight = 0.0;
                }
            }
        }
        self.0.retain(|t| t.weight != 0.0);
    }
}

#[test]
fn test_group_like_terms() {
    let mut a = LinComb(vec![
        LinCombTerm { weight: 1.0, joint_id: NodeIndex::new(0) },
        LinCombTerm { weight: 2.0, joint_id: NodeIndex::new(1) },
        LinCombTerm { weight: 3.0, joint_id: NodeIndex::new(0) },
        LinCombTerm { weight: 4.0, joint_id: NodeIndex::new(0) },
    ]);
    a.group_like_terms_in_place();
    let b = LinComb(vec![
        LinCombTerm { weight: 8.0, joint_id: NodeIndex::new(0) },
        LinCombTerm { weight: 2.0, joint_id: NodeIndex::new(1) },
    ]);
    assert_eq!(a, b);
}
