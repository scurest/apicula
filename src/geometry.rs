use cgmath::Matrix4;
use cgmath::Point2;
use cgmath::Point3;
use cgmath::Transform;
use cgmath::vec4;
use errors::Result;
use index_builder::IndexBuilder;
use joint_builder::JointBuilder;
use joint_builder::JointData;
use nitro::gpu_cmds;
use nitro::mdl::Model;
use nitro::render_cmds;
use std::default::Default;
use std::ops::Range;

#[derive(Debug, Clone)]
pub struct GpuState {
    pub cur_matrix: Matrix4<f64>,
    pub texture_matrix: Matrix4<f64>,
    pub matrix_stack: Vec<Matrix4<f64>>,
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

#[derive(Debug, Clone)]
pub struct DrawCall {
    pub vertex_range: Range<u16>,
    pub index_range: Range<usize>,
    pub mat_id: u8,
    pub mesh_id: u8,
}

#[derive(Debug, Clone)]
pub struct GeometryData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
    pub draw_calls: Vec<DrawCall>,
    pub joint_data: JointData,
}

#[derive(Debug, Clone)]
pub struct Builder<'a, 'b: 'a> {
    model: &'a Model<'b>,
    joint_builder: JointBuilder<'a>,
    gpu: GpuState,
    cur_texture_dim: (u32, u32),
    vertices: Vec<Vertex>,
    ind_builder: IndexBuilder,
    draw_calls: Vec<DrawCall>,
    cur_draw_call: DrawCall,
    next_vertex: Vertex,
}

pub fn build(model: &Model) -> Result<GeometryData> {
    let mut builder = Builder::new(model);
    render_cmds::run_commands(model.render_cmds_cur, &mut builder)?;
    Ok(builder.data())
}

impl GpuState {
    pub fn new() -> GpuState {
        GpuState {
            cur_matrix: Matrix4::one(),
            texture_matrix: Matrix4::one(),
            matrix_stack: vec![Matrix4::one(); 32],
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

impl<'a, 'b: 'a> Builder<'a, 'b> {
    pub fn new(model: &'a Model<'b>) -> Builder<'a, 'b> {
        Builder {
            model: model,
            joint_builder: JointBuilder::new(&model.objects),
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

    pub fn data(self) -> GeometryData {
        GeometryData {
            vertices: self.vertices,
            indices: self.ind_builder.indices,
            draw_calls: self.draw_calls,
            joint_data: self.joint_builder.data(),
        }
    }
}

impl<'a, 'b: 'a> render_cmds::Sink for Builder<'a, 'b> {
    fn load_matrix(&mut self, stack_pos: u8) -> Result<()> {
        self.joint_builder.load_matrix(stack_pos);

        self.gpu.restore(stack_pos);
        Ok(())
    }
    fn store_matrix(&mut self, stack_pos: u8) -> Result<()> {
        self.joint_builder.store_matrix(stack_pos);

        self.gpu.store(stack_pos);
        Ok(())
    }
    fn mul_by_object(&mut self, object_id: u8) -> Result<()> {
        self.joint_builder.mul_by_object(object_id);

        self.gpu.mul_matrix(&self.model.objects[object_id as usize].xform);
        Ok(())
    }
    fn draw(&mut self, mesh_id: u8, mat_id: u8) -> Result<()> {
        let mat = &self.model.materials[mat_id as usize];
        let dim = (mat.width as u32, mat.height as u32);
        self.cur_texture_dim = dim;
        self.gpu.texture_matrix = mat.texture_mat;

        self.begin_draw_call(mesh_id, mat_id);
        gpu_cmds::run_commands(self.model.meshes[mesh_id as usize].commands, self)?;
        self.end_draw_call();

        Ok(())
    }
}

impl<'a, 'b: 'a> gpu_cmds::Sink for Builder<'a, 'b> {
    fn restore(&mut self, idx: u32) {
        self.joint_builder.load_matrix(idx as u8);
        self.gpu.restore(idx as u8);
    }
    fn scale(&mut self, sx: f64, sy: f64, sz: f64) {
        self.gpu.mul_matrix(&Matrix4::from_nonuniform_scale(sx, sy, sz));
    }
    fn begin(&mut self, prim_type: u32) {
        self.ind_builder.begin(prim_type);
    }
    fn end(&mut self) {
        self.ind_builder.end();
        self.cur_draw_call.index_range.end = self.ind_builder.indices.len();
    }
    fn texcoord(&mut self, texcoord: Point2<f64>) {
        let tc = Point2::new(
            texcoord.x / self.cur_texture_dim.0 as f64,
            // TODO: t coordinate seems to be wrong for mirrored textures
            1.0 - texcoord.y / self.cur_texture_dim.1 as f64,
        );
        let tc = self.gpu.texture_matrix * vec4(tc.x, tc.y, 0.0, 0.0);
        self.next_vertex.texcoord = [tc[0] as f32, tc[1] as f32];
    }
    fn color(&mut self, c: Point3<f32>) {
        self.next_vertex.color = [c[0] as f32, c[1] as f32, c[2] as f32];
    }
    fn vertex(&mut self, p: Point3<f64>) {
        self.ind_builder.vertex();
        self.joint_builder.vertex();

        let p = self.gpu.cur_matrix.transform_point(p);
        self.next_vertex.position = [p[0] as f32, p[1] as f32, p[2] as f32];
        self.vertices.push(self.next_vertex);
    }
}
