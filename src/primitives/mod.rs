//! Produce primitive data for a Nitro model.
//!
//! `Primitives` is an intermediate representation of a model's vertex data as
//! buffers of vertices and indices of the sort consumed by eg. `glDrawElements`
//! (as opposed to the raw mesh data which is just a chunk of NDS-specific GPU
//! commands).
//!
//! This is then further consumed by both the viewer and the COLLADA writer.

use cgmath::{Matrix4, Point2, Transform, InnerSpace, vec4, Zero};
use nitro::Model;
use nitro::render_cmds::SkinTerm;
use std::default::Default;
use std::ops::Range;

pub struct Primitives {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
    pub poly_type: PolyType,
    pub draw_calls: Vec<DrawCall>,
}

/// How to interpret the data in the indices buffer.
#[derive(PartialEq, Eq)]
pub enum PolyType {
    /// Every three elements define a triangle.
    Tris,
    /// Every four elements define a polygon. If the fourth element is 0xffff,
    /// it is a triangle given by the first three elements. Otherwise the four
    /// elements define a quad.
    TrisAndQuads,
}

/// Dynamic state for a model (ie. stuff that changes during an animation, as
/// opposed to static state in the Model object).
pub struct DynamicState<'a> {
    /// Object matrices to use.
    pub objects: &'a [Matrix4<f64>],
    /// UV-transform matrices to use for each material.
    pub uv_mats: &'a [Matrix4<f64>],
}

/// Info about the result of a draw call, ie. the result of drawing a piece
/// of a model (a set of GPU commands) while in a particular GPU state
/// (matrix stack, bound material, etc.).
#[derive(Clone)]
pub struct DrawCall {
    /// Executing a draw call for a piece (a set of GPU commands) results in
    /// pushing a set of vertices and indices `primitives.vertices` and
    /// `primitives.indices`. This is the range of of `vertices` that this call
    /// produced.
    pub vertex_range: Range<u16>,
    /// The range of `indices` that this call produced.
    pub index_range: Range<usize>,
    /// The index of the material that was bound when the draw call ran.
    pub mat_id: u8,
    /// The index of the piece that was drawn.
    pub piece_id: u8,
    /// Whether texcoords were set during this call.
    pub used_texcoords: bool,
    /// Whether vertex colors were set during this call.
    pub used_vertex_color: bool,
    /// Whether normals were set during this call.
    pub used_normals: bool,
}

#[derive(Copy, Clone)]
pub struct Vertex {
    pub position: [f32; 3],
    pub texcoord: [f32; 2],
    pub color: [f32; 3],
    pub normal: [f32; 3],
}

impl Default for Vertex {
    fn default() -> Vertex {
        Vertex {
            position: [0.0, 0.0, 0.0],
            texcoord: [0.0, 0.0],
            color: [1.0, 1.0, 1.0],
            normal: [0.0, 0.0, 0.0],
        }
    }
}

// For glium
implement_vertex!(Vertex, position, texcoord, color, normal);

impl Primitives {
    pub fn build(model: &Model, poly_type: PolyType, state: DynamicState) -> Primitives {
        let mut b = Builder::new(model, poly_type, state);
        use nitro::render_cmds::Op;
        for op in &model.render_ops {
            match *op {
                Op::LoadMatrix { stack_pos } => b.load_matrix(stack_pos),
                Op::StoreMatrix { stack_pos } => b.store_matrix(stack_pos),
                Op::MulObject { object_idx } => b.mul_by_object(object_idx),
                Op::Skin { ref terms } => b.blend(&*terms),
                Op::ScaleUp => b.scale_up(),
                Op::ScaleDown => b.scale_down(),
                Op::BindMaterial { material_idx } => b.bind_material(material_idx),
                Op::Draw { piece_idx } => b.draw(piece_idx),
            }
        }
        b.done()
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
    state: DynamicState<'b>,
    poly_type: PolyType,

    gpu: GpuState,
    cur_texture_dim: (u32, u32),
    cur_material: u8,
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
    draw_calls: Vec<DrawCall>,

    first_vertex_in_prim: u16,
    prim_type: u32,

    cur_draw_call: DrawCall,
    next_vertex: Vertex,
}

impl<'a, 'b> Builder<'a, 'b> {
    fn new(model: &'a Model, poly_type: PolyType, state: DynamicState<'b>) -> Builder<'a, 'b> {
        Builder {
            model,
            state,
            poly_type,
            gpu: GpuState::new(),
            vertices: vec![],
            indices: vec![],
            draw_calls: vec![],
            cur_material: 0,
            cur_draw_call: DrawCall {
                vertex_range: 0..0,
                index_range: 0..0,
                mat_id: 0,
                piece_id: 0,
                used_texcoords: false,
                used_vertex_color: false,
                used_normals: false,
            },
            cur_texture_dim: (1,1),
            prim_type: 0,
            first_vertex_in_prim: 0,
            next_vertex: Default::default(),
        }
    }

    fn begin_draw_call(&mut self, piece_id: u8, mat_id: u8) {
        let vert_len = self.vertices.len() as u16;
        let ind_len = self.indices.len();

        // Bind material
        self.gpu.texture_matrix = self.state.uv_mats[mat_id as usize];

        self.cur_draw_call = DrawCall {
            vertex_range: vert_len..vert_len,
            index_range: ind_len..ind_len,
            mat_id,
            piece_id,
            used_texcoords: false,
            used_vertex_color: false,
            used_normals: false,
        };

        self.next_vertex = Default::default();
    }

    fn end_draw_call(&mut self) {
        self.end_prim();

        let len = self.vertices.len() as u16;
        self.cur_draw_call.vertex_range.end = len;
        let len = self.indices.len();
        self.cur_draw_call.index_range.end = len;

        self.draw_calls.push(self.cur_draw_call.clone());
    }

    fn done(self) -> Primitives {
        let vertices = self.vertices;
        let indices = self.indices;
        let poly_type = self.poly_type;
        let draw_calls = self.draw_calls;
        Primitives { vertices, indices, poly_type, draw_calls }
    }

    fn load_matrix(&mut self, stack_pos: u8) {
        self.gpu.restore(stack_pos);
    }

    fn store_matrix(&mut self, stack_pos: u8) {
        self.gpu.store(stack_pos);
    }

    fn mul_by_object(&mut self, object_id: u8) {
        self.gpu.mul_matrix(&self.state.objects[object_id as usize]);
    }

    fn blend(&mut self, terms: &[SkinTerm]) {
        let mut mat = Matrix4::zero();
        for &SkinTerm { weight, stack_pos, inv_bind_idx } in terms {
            let stack_matrix = self.gpu.matrix_stack[stack_pos as usize];
            let inv_bind_matrix = self.model.inv_binds[inv_bind_idx as usize];
            mat += weight as f64 * stack_matrix * inv_bind_matrix;
        }
        self.gpu.cur_matrix = mat;
    }

    fn scale_up(&mut self) {
        self.gpu.mul_matrix(&Matrix4::from_scale(self.model.up_scale));
    }

    fn scale_down(&mut self) {
        self.gpu.mul_matrix(&Matrix4::from_scale(self.model.down_scale));
    }

    fn bind_material(&mut self, material_idx: u8) {
        self.cur_material = material_idx;
    }

    fn draw(&mut self, piece_idx: u8) {
        let cur_material = self.cur_material;

        let mat = &self.model.materials[cur_material as usize];
        let dim = (mat.width as u32, mat.height as u32);
        self.cur_texture_dim = dim;
        self.gpu.texture_matrix = mat.texture_mat;

        self.begin_draw_call(piece_idx, cur_material);

        if mat.diffuse_is_default_vertex_color && mat.diffuse != [1.0, 1.0, 1.0] {
            self.next_vertex.color = mat.diffuse;
            self.cur_draw_call.used_vertex_color = true;
        }

        run_gpu_cmds(self, &self.model.pieces[piece_idx as usize].gpu_commands);
        self.end_draw_call();
    }

    fn begin_prim(&mut self, prim_type: u32) {
        self.end_prim();
        self.first_vertex_in_prim = self.vertices.len() as u16;
        self.prim_type = prim_type;
    }

    fn end_prim(&mut self) {
        let start = self.first_vertex_in_prim;
        let end = self.vertices.len() as u16;

        match self.prim_type {
            0 => {
                // Separate triangles
                //    0      5
                //   / \    / \
                //  1---2  3---4
                let mut i = start;
                while i + 2 < end {
                    self.tri(i, i+1, i+2);
                    i += 3;
                }
            }

            1 => {
                // Separate quads
                //  0---3  6---5
                //  |   |  |   |
                //  1---2  7---4
                let mut i = start;
                while i + 3 < end {
                    self.quad(i, i+1, i+2, i+3);
                    i += 4;
                }
            }

            2 => {
                // Triangle strip
                //  0---2---4
                //   \ / \ / \
                //    1---3---5
                let mut i = start;
                let mut odd = false;
                while i + 2 < end {
                    match odd {
                        false => self.tri(i, i+1, i+2),
                        true => self.tri(i, i+2, i+1),
                    };
                    i += 1;
                    odd = !odd;
                }
            }

            3 => {
                // Quad strip
                //  0---2---4
                //  |   |   |
                //  1---3---5
                let mut i = start;
                while i + 3 < end {
                    self.quad(i, i+1, i+3, i+2);
                    i += 2;
                }
            }

            _ => unreachable!(),
        }

        self.first_vertex_in_prim = self.vertices.len() as u16;
    }

    fn tri(&mut self, i0: u16, i1: u16, i2: u16) {
        match self.poly_type {
            PolyType::Tris =>
                self.indices.extend_from_slice(&[i0, i1, i2]),
            PolyType::TrisAndQuads =>
                self.indices.extend_from_slice(&[i0, i1, i2, 0xffff])
        }
    }

    fn quad(&mut self, i0: u16, i1: u16, i2: u16, i3: u16) {
        match self.poly_type {
            PolyType::Tris => {
                // 0--3   0  0--3
                // |  | = |'. '.|
                // 1--2   1--2  2
                self.indices.extend_from_slice(&[i0, i1, i2, i0, i2, i3])
            }
            PolyType::TrisAndQuads =>
                self.indices.extend_from_slice(&[i0, i1, i2, i3]),
        }
    }
}

fn run_gpu_cmds(b: &mut Builder, commands: &[u8]) {
    use nds::gpu_cmds::{CmdParser, GpuCmd};
    let interpreter = CmdParser::new(commands);

    for cmd_res in interpreter {
        if cmd_res.is_err() { break; }
        match cmd_res.unwrap() {
            GpuCmd::Nop => (),
            GpuCmd::Restore { idx } => b.gpu.restore(idx as u8),
            GpuCmd::Scale { scale: (sx, sy, sz) } => {
                b.gpu.mul_matrix(&Matrix4::from_nonuniform_scale(sx, sy, sz))
            }
            GpuCmd::Begin { prim_type } => b.begin_prim(prim_type),
            GpuCmd::End => b.end_prim(),
            GpuCmd::TexCoord { texcoord } => {
                b.cur_draw_call.used_texcoords = true;

                // Apply texture matrix
                let texcoord = b.gpu.texture_matrix * vec4(texcoord.x, texcoord.y, 0.0, 1.0);

                // Transform into OpenGL-type [0,1]x[0,1] texture space.
                let texcoord = Point2::new(
                    texcoord.x / b.cur_texture_dim.0 as f64,
                    1.0 - texcoord.y / b.cur_texture_dim.1 as f64, // y-down to y-up
                );
                b.next_vertex.texcoord = [texcoord.x as f32, texcoord.y as f32];
            }
            // On the real DS, a normal command computes the lighting factor for
            // the current lights from the normal and sets that as the vertex
            // color. There's not both a vertex color AND a normal. To simulate
            // this with GL-type rendering where there are both, after setting
            // one of the color/normal, we always clear the other.
            GpuCmd::Color { color } => {
                b.cur_draw_call.used_vertex_color = true;
                b.next_vertex.color = [color.x, color.y, color.z];
                b.next_vertex.normal = [0.0, 0.0, 0.0];
            }
            GpuCmd::Normal { normal } => {
                b.cur_draw_call.used_normals = true;
                let n = b.gpu.cur_matrix.transform_vector(normal).normalize();
                b.next_vertex.normal = [n.x as f32, n.y as f32, n.z as f32];
                b.next_vertex.color = [1.0, 1.0, 1.0];
            }
            GpuCmd::Vertex { position } => {
                let p = b.gpu.cur_matrix.transform_point(position);
                b.next_vertex.position = [p.x as f32, p.y as f32, p.z as f32];
                b.vertices.push(b.next_vertex);
            }
        }
    }
}
