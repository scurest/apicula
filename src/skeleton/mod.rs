

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
//! TODO: rewrite documentation after the blend matrix -> inv bind matrix
//! transition.

use cgmath::{ApproxEq, Matrix4, One, SquareMatrix};
use errors::Result;
use nds::gpu_cmds::{self, GpuCmd};
use nitro::{Model, render_cmds};
use util::cur::Cur;
use petgraph::Direction;
use petgraph::stable_graph::StableGraph;
use petgraph::graph::NodeIndex;

pub struct Skeleton {
    pub tree: JointTree,
    pub root: NodeIndex,
    /// The nth entry is the matrix which should be applied to the nth vertex.
    pub vertices: Vec<SymbolicMatrix>,
}

impl Skeleton {
    pub fn build(model: &Model, objects: &[Matrix4<f64>]) -> Result<Skeleton> {
        let mut builder = Builder::new(model, objects);
        render_cmds::run_commands(Cur::new(&model.render_cmds), &mut builder)?;
        Ok(builder.done())
    }
}

/// Tree of joints.
///
/// The convention for edges is that they run _from_ the parent
/// _to_ the child.
pub type JointTree = StableGraph<Node, ()>;

#[derive(Debug, Clone)]
pub struct Node {
    pub transform: Transform,
    /// The world-to-local transform at this node. It could be computed
    /// by walking up the tree and multiplying inverses of the transforms,
    /// but we cache it here for convenience.
    pub inv_bind_matrix: Matrix4<f64>,
    /// Number of vertices that reference this node directly.
    pub ref_count: u32,
}

/// Indicates what kind of matrix (local-to-parent) each node represents.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Transform {
    /// A dummy root. We insert this to ensure the graph is a tree and not
    /// just a forest.
    Root,
    /// The object matrix with the given id.
    Object(u8),
    /// An stack slot that hasn't been assigned to. We treat it as having the
    /// identity as its value. This probably shouldn't show up.
    UninitializedSlot(u8),
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
                SymbolicTerm { weight: 1.0, joint_id }
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

struct Builder<'a, 'b> {
    model: &'a Model,
    objects: &'b [Matrix4<f64>],
    gpu: GpuState,
    skel: Skeleton,
}

struct GpuState {
    cur_matrix: SymbolicMatrix,
    matrix_stack: Vec<Option<SymbolicMatrix>>,
}

impl<'a, 'b> Builder<'a, 'b> {
    pub fn new(model: &'a Model, objects: &'b [Matrix4<f64>]) -> Builder<'a, 'b> {
        let mut tree = StableGraph::new();
        let root = tree.add_node(Node {
            transform: Transform::Root,
            ref_count: 0,
            inv_bind_matrix: Matrix4::one(),
        });
        let vertices = vec![];
        let skel = Skeleton { tree, root, vertices };

        let cur_matrix = SymbolicMatrix::from_joint(root);
        let matrix_stack = vec![None; 32];
        let gpu = GpuState { cur_matrix, matrix_stack };

        Builder { model, objects, gpu, skel }
    }

    pub fn done(self) -> Skeleton {
        cleanup(self.skel)
    }

    pub fn vertex(&mut self) {
        // Update the refcounts
        for &SymbolicTerm { joint_id, .. } in &self.gpu.cur_matrix.terms {
            self.skel.tree[joint_id].ref_count += 1;
        }

        self.skel.vertices.push(self.gpu.cur_matrix.clone());
    }

    /// Premultiplies a symbolic matrix by the given transform.
    fn mul_comb(&mut self, m: SymbolicMatrix, xform: Transform) -> SymbolicMatrix {
        // m * xform = (Σ weight * node) * xform = Σ weight * (node * xform)
        let terms = m.terms.iter()
            .map(|&SymbolicTerm { weight, joint_id }| {
                SymbolicTerm {
                    weight,
                    joint_id: self.find_or_add_child(joint_id, xform)
                }
            })
            .collect();
        SymbolicMatrix { terms }
    }

    /// Returns the matrix at `stack_pos`. Handles unknown stack slots
    /// for you.
    fn get_from_stack(&mut self, stack_pos: u8) -> SymbolicMatrix {
        self.gpu.matrix_stack[stack_pos as usize].clone()
            .unwrap_or_else(|| {
                let root = self.skel.root;
                let node_idx = self.find_or_add_child(
                    root,
                    Transform::UninitializedSlot(stack_pos),
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
        let found = self.skel.tree
            .neighbors_directed(node_id, Direction::Outgoing)
            .find(|&idx| self.skel.tree[idx].transform == transform);
        match found {
            Some(idx) => idx,
            None => {
                // Make a new one.
                let parent_inv_bind = self.skel.tree[node_id].inv_bind_matrix;
                let object_mat = self.transform_to_matrix(transform);
                let inv_object_mat = object_mat.invert().unwrap();
                let inv_bind_matrix = inv_object_mat * parent_inv_bind;
                let new_child = self.skel.tree.add_node(Node {
                    transform,
                    inv_bind_matrix,
                    ref_count: 0,
                });
                self.skel.tree.add_edge(node_id, new_child, ());
                new_child
            }
        }
    }

    /// Gets the local-to-parent matrix for the given transform.
    fn transform_to_matrix(&self, transform: Transform) -> Matrix4<f64> {
        match transform {
            Transform::Root => Matrix4::one(),
            Transform::Object(id) => self.objects[id as usize],
            Transform::UninitializedSlot(_) => Matrix4::one(),
        }
    }
}

impl<'a, 'b> render_cmds::Sink for Builder<'a, 'b> {
    fn load_matrix(&mut self, stack_pos: u8) -> Result<()> {
        self.gpu.cur_matrix = self.get_from_stack(stack_pos);
        Ok(())
    }

    fn store_matrix(&mut self, stack_pos: u8) -> Result<()> {
        self.gpu.matrix_stack[stack_pos as usize] = Some(self.gpu.cur_matrix.clone());
        Ok(())
    }

    fn mul_by_object(&mut self, object_id: u8) -> Result<()> {
        let cur_matrix = self.gpu.cur_matrix.clone();
        let new_matrix = self.mul_comb(cur_matrix, Transform::Object(object_id));
        self.gpu.cur_matrix = new_matrix;
        Ok(())
    }

    fn blend(&mut self, blend_terms: &[(u8, u8, f64)]) -> Result<()> {
        // Set the current matrix to Σ (weight * stack_matrix * inv_bind_matrix).

        // First, check that each stack matrix is a pure matrix (otherwise, it
        // can't have an inverse bind matrix), and if it is, check that the
        // inverse bind we computed is close to the one stored in the model.
        for &(stack_id, inv_bind_id, _weight) in blend_terms {
            let stack_matrix = self.get_from_stack(stack_id);
            if stack_matrix.terms.len() != 1 {
                warn!(
                    "a blended matrix was blended again; this can't be represented \
                    with a COLLADA file. We'll pretend this didn't happen. Your model \
                    may look wrong."
                );
            } else {
                let our_inv_bind = self.skel.tree[stack_matrix.terms[0].joint_id].inv_bind_matrix;
                let stored_inv_bind = self.model.inv_binds[inv_bind_id as usize];
                let close_enough = our_inv_bind.relative_eq(
                    &stored_inv_bind,
                    0.05, // very generous epsilon
                    <Matrix4<f64> as ApproxEq>::default_max_relative(),
                );
                if !close_enough {
                    warn!(
                        "an inverse bind matrix stored in the model file differed \
                        significantly from the inverse bind computed while building \
                        the joint tree; this can't be represented with a COLLADA file. \
                        We'll pretend this didn't happen. Your model may look wrong."
                    );
                }
            }
        }

        // Ok, now assuming the inverse bind matrices are correct, we can
        // just compute Σ (weight * stack_matrix)
        // Distribute over the sum, then group like terms.
        let terms = blend_terms.iter()
            .flat_map(|&(stack_id, _inv_bind_id, weight)| {
                let mut m = self.get_from_stack(stack_id);
                m.mul_scalar_in_place(weight);
                m.terms
            })
            .collect::<Vec<_>>();
        let mut distributed = SymbolicMatrix { terms: terms };
        distributed.group_like_terms_in_place();

        self.gpu.cur_matrix = distributed;

        Ok(())
    }

    fn draw(&mut self, mesh_id: u8, _mat_id: u8) -> Result<()> {
        run_gpu_cmds(self, &self.model.meshes[mesh_id as usize].commands)?;
        Ok(())
    }

    // Don't need to care about these (the scaling is pre-applied to the
    // vertices).
    fn scale_up(&mut self) -> Result<()> { Ok(()) }
    fn scale_down(&mut self) -> Result<()> { Ok(()) }

}

fn run_gpu_cmds(b: &mut Builder, commands: &[u8]) -> Result<()> {
    let interpreter = gpu_cmds::CmdParser::new(commands);

    for cmd_res in interpreter {
        match cmd_res? {
            GpuCmd::Restore { idx } => {
                use nitro::render_cmds::Sink;
                b.load_matrix(idx as u8)?;
            }
            GpuCmd::Vertex { .. } => b.vertex(),
            _ => (),
        }
    }

    Ok(())
}


/// Pass to clean up the skeleton, making it a little nicer for humans.
fn cleanup(mut skel: Skeleton) -> Skeleton {
    /// If `it` yields a single value, returns that value. Otherwise, returns `None`.
    fn first_if_only<I: Iterator>(mut it: I) -> Option<<I as Iterator>::Item> {
        let first = it.next()?;
        match it.next() {
            Some(_) => None,
            None => Some(first),
        }
    }

    //TODO delete leaves that have ref_count == 0?

    // If the dummy root was inserted has a single child, delete the
    // dummy and make its child the new root.
    let root = skel.root;
    let root_child = first_if_only(
        skel.tree.neighbors_directed(root, Direction::Outgoing)
    );
    if let Some(r) = root_child {
        // Make sure no one's using this node...
        if skel.tree[root].ref_count == 0 {
            skel.tree.remove_node(root);
            skel.root = r;
        }
    }

    skel
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
