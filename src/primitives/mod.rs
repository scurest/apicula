//! Produce primitive data for a Nitro model.
//!
//! `Primitives` is an intermediate representation of a model's vertex data as
//! buffers of vertices and indices of the sort consumed by eg. `glDrawElements`
//! (as opposed to the raw mesh data which is just a chunk of NDS-specific GPU
//! commands).
//!
//! This is then further  consumed by both the viewer and the COLLADA writer.

mod index_builder;

use cgmath::{Matrix4, Point2, Transform, vec4, Zero};
use errors::Result;
use primitives::index_builder::IndexBuilder;
use nitro::{Model, render_cmds};
use std::default::Default;
use std::ops::Range;
use util::cur::Cur;

pub struct Primitives {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
    pub draw_calls: Vec<DrawCall>,
}

/// Info about the result of a draw call, ie. the result of drawing a mesh (a set
/// of GPU commands) while in a particular GPU state (matrix stack, bound material,
/// etc.).
#[derive(Clone)]
pub struct DrawCall {
    /// Executing a draw call for a mesh (a set of GPU commands) results in
    /// pushing a set of vertices and indices `primitives.vertices` and
    /// `primitives.indices`. This is the range of of `vertices` that this call
    /// produced.
    pub vertex_range: Range<u16>,
    /// The range of `indices` that this call produced.
    pub index_range: Range<usize>,
    /// The index of the material that was bound when the draw call ran.
    pub mat_id: u8,
    /// The index of the mesh that was drawn.
    pub mesh_id: u8,
}

#[derive(Copy, Clone)]
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

// For glium
implement_vertex!(Vertex, position, texcoord, color);

impl Primitives {
    pub fn build(model: &Model, objects: &[Matrix4<f64>]) -> Result<Primitives> {
        let mut b = Builder::new(model, objects);
        render_cmds::run_commands(Cur::new(&model.render_cmds), &mut b)?;
        Ok(b.done())
    }
}

struct GpuState {
    cur_matrix: Matrix4<f64>,
    matrix_stack: Vec<Matrix4<f64>>,
    /// TODO: texture transforms are barely implemented atm
    texture_matrix: Matrix4<f64>,
}

impl GpuState {
    fn new() -> GpuState {
        GpuState {
            cur_matrix: Matrix4::one(),
            matrix_stack: vec![Matrix4::one(); 32],
            texture_matrix: Matrix4::one(),
        }
    }
    fn restore(&mut self, stack_pos: u8) {
        self.cur_matrix = self.matrix_stack[stack_pos as usize];
    }
    fn store(&mut self, stack_pos: u8) {
        self.matrix_stack[stack_pos as usize] = self.cur_matrix;
    }
    fn mul_matrix(&mut self, mat: &Matrix4<f64>) {
        self.cur_matrix = self.cur_matrix * *mat;
    }
}

struct Builder<'a, 'b> {
    model: &'a Model,
    objects: &'b [Matrix4<f64>],

    gpu: GpuState,
    cur_texture_dim: (u32, u32),
    vertices: Vec<Vertex>,
    ind_builder: IndexBuilder,
    draw_calls: Vec<DrawCall>,

    cur_draw_call: DrawCall,
    next_vertex: Vertex,
}

impl<'a, 'b> Builder<'a, 'b> {
    fn new(model: &'a Model, objects: &'b [Matrix4<f64>]) -> Builder<'a, 'b> {
        Builder {
            model,
            objects,
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

    fn begin_draw_call(&mut self, mesh_id: u8, mat_id: u8) {
        let len = self.vertices.len() as u16;
        self.cur_draw_call.vertex_range = len .. len;
        let len = self.ind_builder.indices.len();
        self.cur_draw_call.index_range = len .. len;
        self.cur_draw_call.mat_id = mat_id;
        self.cur_draw_call.mesh_id = mesh_id;
        self.next_vertex = Default::default();
    }

    fn end_draw_call(&mut self) {
        let len = self.vertices.len() as u16;
        self.cur_draw_call.vertex_range.end = len;
        let len = self.ind_builder.indices.len();
        self.cur_draw_call.index_range.end = len;

        self.draw_calls.push(self.cur_draw_call.clone());
    }

    fn done(self) -> Primitives {
        let vertices = self.vertices;
        let indices = self.ind_builder.indices;
        let draw_calls = self.draw_calls;
        Primitives { vertices, indices, draw_calls }
    }
}

impl<'a, 'b> render_cmds::Sink for Builder<'a, 'b> {
    fn load_matrix(&mut self, stack_pos: u8) -> Result<()> {
        self.gpu.restore(stack_pos);
        Ok(())
    }

    fn store_matrix(&mut self, stack_pos: u8) -> Result<()> {
        self.gpu.store(stack_pos);
        Ok(())
    }

    fn mul_by_object(&mut self, object_id: u8) -> Result<()> {
        self.gpu.mul_matrix(&self.objects[object_id as usize]);
        Ok(())
    }

    fn blend(&mut self, terms: &[(u8, u8, f64)]) -> Result<()> {
        let mut mat = Matrix4::zero();
        for term in terms {
            let weight = term.2;
            let stack_matrix = self.gpu.matrix_stack[term.0 as usize];
            let inv_bind_matrix = self.model.inv_binds[term.1 as usize];
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
        run_gpu_cmds(self, &self.model.meshes[mesh_id as usize].commands)?;
        self.end_draw_call();

        Ok(())
    }
}

fn run_gpu_cmds(b: &mut Builder, commands: &[u8]) -> Result<()> {
    use nds::gpu_cmds::{CmdParser, GpuCmd};
    let interpreter = CmdParser::new(commands);

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
    }

    Ok(())
}
