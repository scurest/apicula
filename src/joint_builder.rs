use cgmath::Matrix4;
use cgmath::One;
use cgmath::SquareMatrix;
use nitro::mdl::Object;
use petgraph::Direction;
use petgraph::Graph;
use petgraph::graph::NodeIndex;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Kind {
    Root,
    Object(u8),
    UndefinedStackSlot(u8),
}

impl Kind {
    pub fn to_matrix(self, objects: &[Object]) -> Matrix4<f64> {
        match self {
            Kind::Root | Kind::UndefinedStackSlot(_) => Matrix4::one(),
            Kind::Object(id) => objects[id as usize].xform,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Weight {
    pub kind: Kind,
    pub inv_bind_matrix: Matrix4<f64>,
}

#[derive(Debug, Clone)]
pub struct JointBuilder<'a> {
    data: JointData,
    objects: &'a [Object],
    cur_matrix: NodeIndex,
    matrix_stack: Vec<Option<NodeIndex>>,
}

#[derive(Debug, Clone)]
pub struct JointData {
    pub tree: Graph<Weight, ()>,
    pub root: NodeIndex,
    pub vertices: Vec<NodeIndex>,
}

impl<'a> JointBuilder<'a> {
    pub fn new(objects: &'a [Object]) -> JointBuilder<'a> {
        let mut tree = Graph::new();
        let root = tree.add_node(Weight {
            kind: Kind::Root,
            inv_bind_matrix: Matrix4::one(),
        });
        JointBuilder {
            data: JointData {
                tree: tree,
                root: root,
                vertices: vec![],
            },
            objects: objects,
            cur_matrix: root,
            matrix_stack: vec![None; 32],
        }
    }
    pub fn data(self) -> JointData {
        self.data
    }
    pub fn load_matrix(&mut self, stack_pos: u8) {
        let slot = self.matrix_stack[stack_pos as usize];
        let idx = match slot {
            Some(idx) => idx,
            None => {
                let n = self.data.tree.add_node(Weight {
                    kind: Kind::UndefinedStackSlot(stack_pos),
                    inv_bind_matrix: Matrix4::one(),
                });
                self.data.tree.add_edge(self.data.root, n, ());
                n
            }
        };
        self.cur_matrix = idx;
    }
    pub fn mul_by_object(&mut self, object_id: u8) {
        let cur_matrix = self.cur_matrix;
        let new = self.find_or_add_child(cur_matrix, Kind::Object(object_id));
        self.cur_matrix = new;
    }
    pub fn store_matrix(&mut self, stack_pos: u8) {
        self.matrix_stack[stack_pos as usize] = Some(self.cur_matrix);
    }
    fn find_or_add_child(&mut self, at: NodeIndex, kind: Kind) -> NodeIndex {
        let found = self.data.tree
            .neighbors_directed(at, Direction::Outgoing)
            .find(|&idx| self.data.tree[idx].kind == kind);
        match found {
            Some(idx) => idx,
            None => {
                let parent_inv_bind = self.data.tree[at].inv_bind_matrix;
                let object_mat = kind.to_matrix(self.objects);
                let inv_object_mat = object_mat.invert().expect("inverse did not exist");
                let new_child = self.data.tree.add_node(Weight {
                    kind: kind,
                    inv_bind_matrix: inv_object_mat * parent_inv_bind,
                });
                self.data.tree.add_edge(at, new_child, ());
                new_child
            }
        }
    }
    pub fn vertex(&mut self) {
        let cur_matrix = self.cur_matrix;
        self.data.vertices.push(cur_matrix);
    }
}
