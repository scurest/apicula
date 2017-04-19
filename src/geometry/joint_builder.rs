//! Build the joint tree for a model.
//!
//! Models are skinned on the DS with a series of imperative commands
//! referencing a flat array of matrices. COLLADA, on the other hand,
//! uses a declarative tree of joint nodes and vertex weights. This
//! module corrects this impedance mismatch, consuming the imperative
//! commands and building a joint tree that will produce the same effect.
//!
//! --------
//!
//! In brief, in COLLADA, we have a tree of "joints". Each joint is
//! associated with a matrix that transform from its local space to
//! its parent's space. The local-to-world transform for the joint
//! is the concatenation of its matrix with all of its ancestors' up
//! to the root. Each vertex in the model is then transformed by a
//! weighted combination of the joints.
//!
//!        A        For example, in this tree, a vertex which was
//!       / \       skinned by E with weight 0.2 and by B with
//!      B   C      weight 0.8 would be transformed into world space
//!         / \     by 0.2 A * C * E + 0.8 B * A.
//!        D   E
//!
//! On the DS, each vertex is transformed into world space by the current
//! matrix on the GPU. The render commands in an MDL allow different
//! matrices to be loaded into GOU stack slots. The commands are such
//! that the general form of a matrix that they can create is
//! Σ (scalar) * (pure matrix), where a "pure matrix" is a composition
//! of object matrices and blend matrices.
//!
//! Comparing these, what we have to do is obvious. We'll make the joint
//! tree out of the pure matrices--a product (obj1)*(obj2)*(blend1) will
//! just become a set of ancestors (obj1)->(obj2)->(blend1)--and the
//! scalars will become the vertex weights.
//!
//! We interpret the render commands, but using symbolic algebra instead
//! of computing numerically, so a GPU matrix will be a symbolic expression
//! for the weighted sum of pure matrices. The pure matrices are represented
//! as indices into the joint tree that we are building up as we execute the
//! commands.

use cgmath::Matrix4;
use cgmath::One;
use cgmath::SquareMatrix;
use nitro::mdl::Model;
use petgraph::Direction;
use petgraph::stable_graph::StableGraph;
use petgraph::graph::NodeIndex;
use util::first_if_only::first_if_only;

/// Tree of joints. Each joint represents a "meaningful" matrix. The
/// local-to-world transform of a node is the
///
/// The convention for edges is that they run _from_ the parent
/// _to_ the child.
pub type JointTree = StableGraph<Node, ()>;

#[derive(Debug, Clone)]
pub struct Node {
    pub transform: Transform,
    /// Number of vertices that reference this node directly.
    pub ref_count: u32,
    /// The world-to-local transform at this node. It could be computed
    /// by walking up the tree and multiplying inverses of the transforms,
    /// but we cache it here for convenience.
    pub inv_bind_matrix: Matrix4<f64>,
}

/// Indicates what kind of matrix (local-to-parent) each node represents.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Transform {
    /// A dummy root. We insert this to ensure the graph is a tree and not
    /// just a forest.
    Root,
    /// The objects matrix with the given index. We're computing the bind pose,
    /// so the value of these is what is given in the MDL file.
    Object(u8),
    /// The blend matrix in the MDL file with the given index.
    Blend(u8),
    /// An unassigned stack slot. We treat it as having the identity as its
    /// value. If this shows up, there's probably a bug.
    UnknownStackSlot(u8),
}

/// A linear combination of pure matrices. Basically the free linear space
/// on the set of indices into the joint tree. This is the general form of
/// the matrices manipulated by the GPU.
///
/// TODO: the great majority of these are a single term and I have never
/// seen more than three terms. Use a SmallVec?
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolicMatrix {
    pub terms: Vec<SymbolicTerm>
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolicTerm {
    pub weight: f64,
    pub joint_id: NodeIndex,
}

impl SymbolicMatrix {
    /// Returns a linear combination consisting only of the given
    /// node matrix (this is monadic `pure`).
    fn from_joint(joint_id: NodeIndex) -> SymbolicMatrix {
        SymbolicMatrix {
            terms: vec![
                SymbolicTerm { weight: 1.0, joint_id: joint_id }
            ],
        }
    }

    fn mul_scalar_in_place(&mut self, lam: f64) {
        for term in &mut self.terms {
            term.weight *= lam;
        }
    }

    fn group_like_terms_in_place(&mut self) {
        // For every term t, move the weight from any subsequent like
        // term into the factor for t. Then remove all terms with weight
        // zero. Don't worry about this being quadratic, since these
        // vectors are tiny.
        for i in 0..self.terms.len() {
            for j in i+1..self.terms.len() {
                if self.terms[i].joint_id == self.terms[j].joint_id {
                    self.terms[i].weight += self.terms[j].weight;
                    self.terms[j].weight = 0.0;
                }
            }
        }
        self.terms.retain(|term| term.weight != 0.0);
    }
}


#[derive(Debug, Clone)]
pub struct JointBuilder<'a, 'b: 'a> {
    data: JointData,
    model: &'a Model<'b>,

    /// GPU's current matrix.
    cur_matrix: SymbolicMatrix,
    /// The GPU's matrix stack. If an entry contains `None`, it means it hasn't been
    /// written to, so we don't know its value.
    matrix_stack: Vec<Option<SymbolicMatrix>>,
}

#[derive(Debug, Clone)]
pub struct JointData {
    pub tree: JointTree,
    pub root: NodeIndex,
    /// The nth entry is the matrix which should be applied to the nth vertex.
    pub vertices: Vec<SymbolicMatrix>,
}

impl<'a, 'b: 'a> JointBuilder<'a, 'b> {
    pub fn new(model: &'a Model<'b>) -> JointBuilder<'a, 'b> {
        let mut tree = StableGraph::new();

        let root = tree.add_node(Node {
            transform: Transform::Root,
            ref_count: 0,
            inv_bind_matrix: Matrix4::one(),
        });

        let cur_matrix = SymbolicMatrix::from_joint(root);
        let matrix_stack = vec![None; 32];

        let data = JointData {
            tree: tree,
            root: root,
            vertices: vec![],
        };

        JointBuilder {
            data: data,
            model: model,
            cur_matrix: cur_matrix,
            matrix_stack: matrix_stack,
        }
    }

    pub fn data(self) -> JointData {
        cleanup(self.data)
    }

    pub fn load_matrix(&mut self, stack_pos: u8) {
        self.cur_matrix = self.get_from_stack(stack_pos);
    }

    pub fn mul_by_object(&mut self, object_id: u8) {
        let cur_matrix = self.cur_matrix.clone();
        let new_matrix = self.mul_comb(cur_matrix, Transform::Object(object_id));
        self.cur_matrix = new_matrix;
    }

    pub fn store_matrix(&mut self, stack_pos: u8) {
        self.matrix_stack[stack_pos as usize] = Some(self.cur_matrix.clone());
    }

    pub fn blend(&mut self, blend_terms: &[(u8, u8, f64)]) {
        // Set the current matrix to Σ (weight * stack_matrix * blend_matrix).

        // First, distribute the sum over the linear combination for each
        // stack matrix.
        let terms = blend_terms.iter()
            .flat_map(|&(stack_id, blend_id, weight)| {
                let stack_matrix = self.get_from_stack(stack_id);
                let mut res = self.mul_comb(stack_matrix, Transform::Blend(blend_id));
                res.mul_scalar_in_place(weight);
                res.terms
            })
            .collect::<Vec<_>>();
        let mut distributed = SymbolicMatrix { terms: terms };

        // Now, group like terms.
        distributed.group_like_terms_in_place();
        self.cur_matrix = distributed;
    }

    pub fn vertex(&mut self) {
        // Update the refcounts
        for &SymbolicTerm { joint_id, .. } in &self.cur_matrix.terms {
            self.data.tree[joint_id].ref_count += 1;
        }

        self.data.vertices.push(self.cur_matrix.clone());
    }

    /// Premultiplies a symbolic matrix by the given transform.
    fn mul_comb(&mut self, v: SymbolicMatrix, xform: Transform) -> SymbolicMatrix {
        // v * xform = (Σ weight * node) * xform = Σ weight * (node * xform)
        let terms = v.terms.iter()
            .map(|&SymbolicTerm { weight, joint_id }| {
                SymbolicTerm {
                    weight: weight,
                    joint_id: self.find_or_add_child(joint_id, xform)
                }
            })
            .collect();
        SymbolicMatrix { terms: terms }
    }

    /// Returns the matrix at `stack_pos`. Handles unknown stack slots
    /// for you.
    fn get_from_stack(&mut self, stack_pos: u8) -> SymbolicMatrix {
        self.matrix_stack[stack_pos as usize].clone()
            .unwrap_or_else(|| {
                let root = self.data.root;
                let node_idx = self.find_or_add_child(
                    root,
                    Transform::UnknownStackSlot(stack_pos),
                );
                SymbolicMatrix::from_joint(node_idx)
            })
    }

    /// Returns a child of the node `node_id` with the given transform value. If
    /// one exists (there should be at most one), it will be used; otherwise, one
    /// is created.
    ///
    /// This corresponds to multiplying the matrix represented by `node_id` by the
    /// given transform.
    fn find_or_add_child(&mut self, node_id: NodeIndex, transform: Transform) -> NodeIndex {
        let found = self.data.tree
            .neighbors_directed(node_id, Direction::Outgoing)
            .find(|&idx| self.data.tree[idx].transform == transform);
        match found {
            Some(idx) => idx,
            None => {
                // Make a new one.
                let parent_inv_bind = self.data.tree[node_id].inv_bind_matrix;
                let object_mat = self.transform_to_matrix(transform);
                let inv_object_mat = object_mat.invert()
                    .unwrap_or_else(|| {
                        warn!("while building inverse bind matrix, a non-\
                            invertible matrix was encountered");
                        Matrix4::one() // try to keep going...
                    });
                let inv_bind_matrix = inv_object_mat * parent_inv_bind;
                let new_child = self.data.tree.add_node(Node {
                    transform: transform,
                    ref_count: 0,
                    inv_bind_matrix: inv_bind_matrix,
                });
                self.data.tree.add_edge(node_id, new_child, ());
                new_child
            }
        }
    }

    /// Gets the local-to-parent matrix for the given transform.
    fn transform_to_matrix(&self, transform: Transform) -> Matrix4<f64> {
        match transform {
            Transform::Root => Matrix4::one(),
            Transform::Object(id) => self.model.objects[id as usize].xform,
            Transform::Blend(id) => self.model.blend_matrices[id as usize].0,
            Transform::UnknownStackSlot(_) => Matrix4::one(),
        }
    }
}

/// Pass to clean up the joint data, making it a little nicer for human users.
fn cleanup(mut data: JointData) -> JointData {
    // If the dummy root was inserted has a single child, delete the
    // dummy and make its child the new root.
    let root = data.root;
    let root_child = first_if_only(
        data.tree.neighbors_directed(root, Direction::Outgoing)
    );
    if let Some(r) = root_child {
        // Make sure no one's using this node...
        if data.tree[root].ref_count == 0 {
            data.tree.remove_node(root);
            data.root = r;
        }
    }

    data
}

#[test]
fn test_group_like_terms() {
    let mut a = SymbolicMatrix { terms: vec![
        SymbolicTerm { weight: 1.0, joint_id: NodeIndex::new(0) },
        SymbolicTerm { weight: 2.0, joint_id: NodeIndex::new(1) },
        SymbolicTerm { weight: 3.0, joint_id: NodeIndex::new(0) },
        SymbolicTerm { weight: 4.0, joint_id: NodeIndex::new(0) },
        SymbolicTerm { weight: 0.0, joint_id: NodeIndex::new(2) },
    ]};
    a.group_like_terms_in_place();
    let b = SymbolicMatrix { terms: vec![
        SymbolicTerm { weight: 8.0, joint_id: NodeIndex::new(0) },
        SymbolicTerm { weight: 2.0, joint_id: NodeIndex::new(1) },
    ]};
    assert_eq!(a, b);
}
