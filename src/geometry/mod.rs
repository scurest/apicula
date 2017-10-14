//! Consumes Nitro data and produces geometry data.
//!
//! This modules produces an intermediate data structure used by the viewer and
//! converter, called `GeometryData`, which converts between the NDS GPU commands
//! and Nitro rendering commands and a standard geometry representation with vertex
//! buffers, triangle indices, etc.
//!
//! The geometry data can be produced either with or without joint data. The joint
//! tree is used by the COLLADA exporter, but is expensive to build, so we provide
//! the option to forego it for the viewer, which rebuilds this intermediate
//! structure every time a model animation advances.

pub mod joint_builder;
mod index_builder;

use cgmath::Matrix4;
use cgmath::Zero;
use cgmath::Point2;
use cgmath::Transform;
use cgmath::vec4;
use errors::Result;
use geometry::index_builder::IndexBuilder;
use geometry::joint_builder::JointBuilder;
use geometry::joint_builder::JointData;
use nitro::gpu_cmds;
use nitro::gpu_cmds::GpuCmd;
use nitro::mdl::InvBindMatrixPair;
use nitro::mdl::Model;
use nitro::mdl::render_cmds;
use std::default::Default;
use std::ops::Range;

#[derive(Debug, Clone)]
pub struct GeometryDataWithJoints {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
    pub draw_calls: Vec<DrawCall>,
    pub joint_data: JointData,
    /// The object matrices for the bind pose. The same as the object
    /// matrices in the model file, but tweaked to be invertible.
    pub objects: Vec<Matrix4<f64>>,
}

#[derive(Debug, Clone)]
pub struct GeometryDataWithoutJoints {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
    pub draw_calls: Vec<DrawCall>,
}

#[derive(Debug, Clone)]
pub struct GpuState {
    pub cur_matrix: Matrix4<f64>,
    pub matrix_stack: Vec<Matrix4<f64>>,
    /// TODO: texture transforms are barely implemented atm
    pub texture_matrix: Matrix4<f64>,
}

impl GpuState {
    pub fn new() -> GpuState {
        GpuState {
            cur_matrix: Matrix4::one(),
            matrix_stack: vec![Matrix4::one(); 32],
            texture_matrix: Matrix4::one(),
        }
    }
    pub fn restore(&mut self, stack_pos: u8) {
        self.cur_matrix = self.matrix_stack[stack_pos as usize];
    }
    pub fn store(&mut self, stack_pos: u8) {
        self.matrix_stack[stack_pos as usize] = self.cur_matrix;
    }
    pub fn mul_matrix(&mut self, mat: &Matrix4<f64>) {
        self.cur_matrix = self.cur_matrix * *mat;
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Vertex {
    pub position: [f32; 3],
    pub texcoord: [f32; 2],
    pub color: [f32; 3],
}

impl Default for Vertex {
    fn default() -> Vertex {
        Vertex {
            position: [0.0, 0.0, 0.0],
            texcoord: [0.0, 0.0],
            color: [1.0, 1.0, 1.0],
        }
    }
}

/// Info about the result of a draw call, ie. the result of drawing a mesh (a set
/// of GPU commands) while in a particular GPU state (matrix stack, bound material,
/// etc.).
#[derive(Debug, Clone)]
pub struct DrawCall {
    /// Executing a draw call for a mesh (a set of GPU commands) results in pushing
    /// a set of vertices and indices onto those buffers. This is the range of
    /// of `vertices` that this call produced.
    pub vertex_range: Range<u16>,
    /// The range of `indices` that this call produced.
    pub index_range: Range<usize>,
    /// The index of the material that was bound when the draw call ran.
    pub mat_id: u8,
    /// The index of the mesh that was drawn.
    pub mesh_id: u8,
}

#[derive(Debug, Clone)]
pub struct Builder<'a, 'b: 'a, 'c> {
    model: &'a Model<'b>,
    objects: &'c [Matrix4<f64>],
    joint_builder: Option<JointBuilder<'a, 'b, 'c>>,
    gpu: GpuState,
    cur_texture_dim: (u32, u32),
    vertices: Vec<Vertex>,
    ind_builder: IndexBuilder,
    draw_calls: Vec<DrawCall>,
    cur_draw_call: DrawCall,
    next_vertex: Vertex,
}

pub fn build_with_joints(
    model: &Model,
    objects: &[Matrix4<f64>]
) -> Result<GeometryDataWithJoints> {
    /// Make singular matrices invertible by perturbing them very slightly,
    /// if necessary.
    fn make_invertible(m: &Matrix4<f64>) -> Matrix4<f64> {
        use cgmath::SquareMatrix;
        if m.is_invertible() {
            m.clone()
        } else {
            // Bump very slightly along the diagonal.
            let m2 = m + Matrix4::from_scale(0.00001);
            if m2.is_invertible() {
                m2
            } else {
                // #&$@$!! Give up.
                warn!("non-invertible object encountered while building \
                       joint tree; using identity instead");
                info!("namely, {:#?}", m);
                Matrix4::one()
            }
        }
    }

    // The object matrices need to be invertible to form the inverse
    // bind matrices for the joint tree.
    let objects = objects
            .iter()
            .map(|mat| make_invertible(mat))
            .collect::<Vec<Matrix4<f64>>>();
    let data = {
        let joint_builder = JointBuilder::new(model, &objects);
        let mut builder = Builder::new(model, &objects[..], Some(joint_builder));
        render_cmds::run_commands(model.render_cmds_cur, &mut builder)?;
        builder.data()
    };
    Ok(GeometryDataWithJoints {
        vertices: data.0.vertices,
        indices: data.0.indices,
        draw_calls: data.0.draw_calls,
        joint_data: data.1.unwrap(),
        objects,
    })
}

pub fn build_without_joints(
    model: &Model,
    objects: &[Matrix4<f64>]
) -> Result<GeometryDataWithoutJoints> {
    let mut builder = Builder::new(model, objects, None);
    render_cmds::run_commands(model.render_cmds_cur, &mut builder)?;
    Ok(builder.data().0)
}

impl<'a, 'b: 'a, 'c> Builder<'a, 'b, 'c> {
    pub fn new(
        model: &'a Model<'b>,
        objects: &'c [Matrix4<f64>],
        joint_builder: Option<JointBuilder<'a, 'b, 'c>>
    ) -> Builder<'a, 'b, 'c> {
        Builder {
            model,
            objects,
            joint_builder,
            gpu: GpuState::new(),
            vertices: vec![],
            ind_builder: IndexBuilder::new(),
            draw_calls: vec![],
            cur_draw_call: DrawCall {
                vertex_range: 0..0,
                index_range: 0..0,
                mat_id: 0,
                mesh_id: 0,
            },
            cur_texture_dim: (1,1),
            next_vertex: Default::default(),
        }
    }

    pub fn begin_draw_call(&mut self, mesh_id: u8, mat_id: u8) {
        let len = self.vertices.len() as u16;
        self.cur_draw_call.vertex_range = len .. len;
        let len = self.ind_builder.indices.len();
        self.cur_draw_call.index_range = len .. len;
        self.cur_draw_call.mat_id = mat_id;
        self.cur_draw_call.mesh_id = mesh_id;
        self.next_vertex = Default::default();
    }

    pub fn end_draw_call(&mut self) {
        let len = self.vertices.len() as u16;
        self.cur_draw_call.vertex_range.end = len;
        let len = self.ind_builder.indices.len();
        self.cur_draw_call.index_range.end = len;

        self.draw_calls.push(self.cur_draw_call.clone());
    }

    pub fn data(self) -> (GeometryDataWithoutJoints, Option<JointData>) {
        (
            GeometryDataWithoutJoints {
                vertices: self.vertices,
                indices: self.ind_builder.indices,
                draw_calls: self.draw_calls,
            },
            self.joint_builder.map(|b| b.data()),
        )
    }
}

impl<'a, 'b: 'a, 'c> render_cmds::Sink for Builder<'a, 'b, 'c> {
    fn load_matrix(&mut self, stack_pos: u8) -> Result<()> {
        if let Some(ref mut b) = self.joint_builder {
            b.load_matrix(stack_pos);
        }

        self.gpu.restore(stack_pos);
        Ok(())
    }

    fn store_matrix(&mut self, stack_pos: u8) -> Result<()> {
        if let Some(ref mut b) = self.joint_builder {
            b.store_matrix(stack_pos);
        }

        self.gpu.store(stack_pos);
        Ok(())
    }

    fn mul_by_object(&mut self, object_id: u8) -> Result<()> {
        if let Some(ref mut b) = self.joint_builder {
            b.mul_by_object(object_id);
        }

        self.gpu.mul_matrix(&self.objects[object_id as usize]);
        Ok(())
    }

    fn blend(&mut self, terms: &[(u8, u8, f64)]) -> Result<()> {
        if let Some(ref mut b) = self.joint_builder {
            b.blend(terms);
        }

        let mut mat = Matrix4::zero();
        for term in terms {
            let weight = term.2;
            let stack_matrix = self.gpu.matrix_stack[term.0 as usize];
            let inv_bind_matrix = self.model.inv_bind_matrices_cur
                .nth::<InvBindMatrixPair>(term.1 as usize)?.0;
            mat += weight * stack_matrix * inv_bind_matrix;
        }
        self.gpu.cur_matrix = mat;

        Ok(())
    }

    fn scale_up(&mut self) -> Result<()> {
        self.gpu.mul_matrix(&Matrix4::from_scale(self.model.up_scale));
        Ok(())
    }

    fn scale_down(&mut self) -> Result<()> {
        self.gpu.mul_matrix(&Matrix4::from_scale(self.model.down_scale));
        Ok(())
    }

    fn draw(&mut self, mesh_id: u8, mat_id: u8) -> Result<()> {
        let mat = &self.model.materials[mat_id as usize];
        let dim = (mat.width as u32, mat.height as u32);
        self.cur_texture_dim = dim;
        self.gpu.texture_matrix = mat.texture_mat;

        self.begin_draw_call(mesh_id, mat_id);
        run_gpu_cmds(self, self.model.meshes[mesh_id as usize].commands)?;
        self.end_draw_call();

        Ok(())
    }
}

fn run_gpu_cmds(b: &mut Builder, commands: &[u8]) -> Result<()> {
    let interpreter = gpu_cmds::CmdParser::new(commands);

    for cmd_res in interpreter {
        let cmd = cmd_res?;

        match cmd {
            GpuCmd::Nop => (),
            GpuCmd::Restore { idx } => b.gpu.restore(idx as u8),
            GpuCmd::Scale { scale: (sx, sy, sz) } => {
                b.gpu.mul_matrix(&Matrix4::from_nonuniform_scale(sx, sy, sz))
            }
            GpuCmd::Begin { prim_type } => b.ind_builder.begin(prim_type),
            GpuCmd::End => {
                b.ind_builder.end();
                b.cur_draw_call.index_range.end = b.ind_builder.indices.len();
            }
            GpuCmd::TexCoord { texcoord } => {
                // Transform into OpenGL-type [0,1]x[0,1] texture space.
                let texcoord = Point2::new(
                    texcoord.x / b.cur_texture_dim.0 as f64,
                    1.0 - texcoord.y / b.cur_texture_dim.1 as f64, // y-down to y-up
                );
                let texcoord = b.gpu.texture_matrix * vec4(texcoord.x, texcoord.y, 0.0, 0.0);
                b.next_vertex.texcoord = [texcoord.x as f32, texcoord.y as f32];
            }
            GpuCmd::Color { color } => b.next_vertex.color = [color.x, color.y, color.z],
            GpuCmd::Normal { .. } => (), // unimplemented
            GpuCmd::Vertex { position } => {
                b.ind_builder.vertex();

                let p = b.gpu.cur_matrix.transform_point(position);
                b.next_vertex.position = [p.x as f32, p.y as f32, p.z as f32];
                b.vertices.push(b.next_vertex);
            }
        }

        // Also send the command to the joint builder if we have one.
        if let Some(ref mut joint_builder) = b.joint_builder {
            joint_builder.run_gpu_cmd(cmd);
        }
    }

    Ok(())
}
