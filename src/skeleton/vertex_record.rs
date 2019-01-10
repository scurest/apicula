//! Abstract interpretation of the render commands to discover what symbolic
//! matrix is applied to each vertex.

use super::symbolic_matrix::{SMatrix, AMatrix};
use nitro::Model;
use nitro::render_cmds::SkinTerm;

type MatrixIdx = u16;

/// Records what symbolic matrix applies to each vertex.
pub struct VertexRecord {
    /// A list of all the symbolic matrices computed in the course of drawing
    /// the model.
    pub matrices: Vec<AMatrix>,
    /// For each vertex, which matrix in the above list is applied to it.
    pub vertices: Vec<MatrixIdx>,
}

impl VertexRecord {
    pub fn build_for_model(model: &Model) -> VertexRecord {
        let mut b = Builder::new(model);
        use nitro::render_cmds::Op;
        for op in &model.render_ops {
            match *op {
                Op::LoadMatrix { stack_pos } => b.load_matrix(stack_pos),
                Op::StoreMatrix { stack_pos } => b.store_matrix(stack_pos),
                Op::MulObject { object_idx } => b.mul_object(object_idx),
                Op::Skin { ref terms } => b.skin(&*terms),
                Op::ScaleUp => b.scale_up(),
                Op::ScaleDown => b.scale_down(),
                Op::BindMaterial { .. } => (),
                Op::Draw { mesh_idx } => b.draw(mesh_idx),
            }
        }
        b.vr
    }
}

struct Builder<'a> {
    model: &'a Model,
    vr: VertexRecord,

    cur_matrix: MatrixIdx,
    matrix_stack: Vec<Option<MatrixIdx>>,
}

impl<'a> Builder<'a> {
    fn new(model: &Model) -> Builder {
        Builder {
            model,
            vr: VertexRecord {
                matrices: vec![AMatrix::one()],
                vertices: vec![],
            },
            cur_matrix: 0,
            matrix_stack: vec![None; 32],
        }
    }

    /// Add a new AMatrix to the record, returning its index.
    fn add_matrix(&mut self, mat: AMatrix) -> MatrixIdx {
        self.vr.matrices.push(mat);
        (self.vr.matrices.len() - 1) as MatrixIdx
    }

    fn fetch_from_stack(&mut self, stack_pos: u8) -> MatrixIdx {
        // If the slot is uninitialized, make a new Uninitialized SMatrix for
        // it.
        if self.matrix_stack[stack_pos as usize].is_none() {
            let uninit = SMatrix::Uninitialized { stack_pos }.into();
            let uninit_idx = self.add_matrix(uninit);
            self.matrix_stack[stack_pos as usize] = Some(uninit_idx);
        }
        self.matrix_stack[stack_pos as usize].unwrap()
    }

    fn load_matrix(&mut self, stack_pos: u8) {
        let idx = self.fetch_from_stack(stack_pos);
        self.cur_matrix = idx;
    }

    fn store_matrix(&mut self, stack_pos: u8) {
        self.matrix_stack[stack_pos as usize] = Some(self.cur_matrix);
    }

    fn mul_object(&mut self, object_idx: u8) {
        let mut mat = self.vr.matrices[self.cur_matrix as usize].clone();
        mat *= SMatrix::Object { object_idx };
        self.cur_matrix = self.add_matrix(mat);
    }

    fn skin(&mut self, terms: &[SkinTerm]) {
        let mut acc = AMatrix::zero();
        for term in terms {
            // weight * stack[stack_pos] * inv_binds[inv_bind_idx]
            let mat_idx = self.fetch_from_stack(term.stack_pos);
            let mut mat = self.vr.matrices[mat_idx as usize].clone();
            mat *= SMatrix::InvBind { inv_bind_idx: term.inv_bind_idx };
            mat *= term.weight;

            acc += mat
        }
        let mat_idx = self.add_matrix(acc);
        self.cur_matrix = mat_idx;
    }

    // NOTE: Ignored for now, which is incorrect, but they IME don't end up
    // affecting the final skeleton (because they end up in the "longest suffix
    // of constant factors"; see joint_tree) so it doesn't matter much.
    fn scale_up(&mut self) { }
    fn scale_down(&mut self) { }

    fn draw(&mut self, mesh_idx: u8) {
        let mesh = &self.model.meshes[mesh_idx as usize];
        use nds::gpu_cmds::{CmdParser, GpuCmd};
        let interpreter = CmdParser::new(&mesh.gpu_commands);

        for cmd_res in interpreter {
            if cmd_res.is_err() { break; }
            match cmd_res.unwrap() {
                GpuCmd::Restore { idx } => self.load_matrix(idx as u8),
                // Again, ignore scalings.
                GpuCmd::Scale { .. } => (),
                GpuCmd::Vertex { .. } => {
                    let cur_matrix = self.cur_matrix;
                    self.vr.vertices.push(cur_matrix)
                }
                _ => (),
            }
        }
    }
}
