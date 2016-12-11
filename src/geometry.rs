use cgmath::Point2;
use cgmath::Point3;
use gfx;
use std::default::Default;
use std::ops::Range;

#[derive(Debug, Copy, Clone)]
pub struct Vertex {
    pub position: [f32; 3],
    pub texcoord: [f32; 2],
    pub color: [f32; 3],
}

#[derive(Debug, Clone)]
pub struct MeshRange {
    pub vertex_range: Range<u16>,
    pub index_range: Range<usize>,
    pub mat_id: u8,
}

#[derive(Debug, Clone, Default)]
pub struct GeometryData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
    pub mesh_ranges: Vec<MeshRange>,
}

#[derive(Debug, Clone)]
pub struct Sink {
    pub data: GeometryData,
    pub cur_texture_dim: (u32, u32),
    cur_mesh_range: MeshRange,
    cur_prim_type: u32,
    next_texcoord: Point2<f64>,
    next_color: Point3<f32>,
}

impl Sink {
    pub fn new() -> Sink {
        Sink {
            data: Default::default(),
            cur_mesh_range: MeshRange {
                vertex_range: 0..0,
                index_range: 0..0,
                mat_id: 0,
            },
            cur_prim_type: 0,
            cur_texture_dim: (1,1),
            next_texcoord: Point2::new(0.0, 0.0),
            next_color: Point3::new(1.0, 1.0, 1.0),
        }
    }
    pub fn begin_mesh(&mut self, mat_id: u8) {
        let len = self.data.indices.len();
        self.cur_mesh_range.index_range = len .. len;
        self.cur_mesh_range.mat_id = mat_id;
        self.next_texcoord = Point2::new(0.0, 0.0);
        self.next_color = Point3::new(1.0, 1.0, 1.0);
    }
    pub fn end_mesh(&mut self) {
        self.data.mesh_ranges.push(self.cur_mesh_range.clone());
    }
}

impl gfx::Sink for Sink {
    fn begin(&mut self, prim_type: u32) {
        self.cur_prim_type = prim_type;
        let len = self.data.vertices.len() as u16;
        self.cur_mesh_range.vertex_range = len .. len;
    }
    fn end(&mut self) {
        let r = self.cur_mesh_range.vertex_range.clone();
        match self.cur_prim_type {
            0 => {
                // Seperate triangles
                let mut i = r.start;
                while i != r.end {
                    self.data.indices.extend_from_slice(&[
                        i, i+1, i+2,
                    ]);
                    i += 3;
                }
            }
            1 => {
                // Seperate quads
                let mut i = r.start;
                while i != r.end {
                    self.data.indices.extend_from_slice(&[
                        i, i+1, i+2,
                        i+2, i+3, i,
                    ]);
                    i += 4;
                }
            }
            2 => {
                // Triangle strip
                let mut i = r.start;
                if i != r.end {
                    self.data.indices.extend_from_slice(&[
                        i, i+1, i+2,
                    ]);
                    i += 3;
                }
                while i != r.end {
                    self.data.indices.extend_from_slice(&[
                        i, i-1, i-2,
                    ]);
                    i += 1;
                    if i == r.end { break; }
                    self.data.indices.extend_from_slice(&[
                        i, i-2, i-1,
                    ]);
                    i += 1;
                }
            }
            3 => {
                // Quad strip
                let mut i = r.start;
                if i != r.end {
                    self.data.indices.extend_from_slice(&[
                        i, i+1, i+2,
                        i+2, i+1, i+3,
                    ]);
                    i += 4;
                }
                while i != r.end {
                    self.data.indices.extend_from_slice(&[
                        i-2, i-1, i,
                        i, i-1, i+1,
                    ]);
                    i += 2;
                }
            }
            _ => unreachable!(),
        }
        self.cur_mesh_range.index_range.end = self.data.indices.len();
    }
    fn texcoord(&mut self, texcoord: Point2<f64>) {
        self.next_texcoord = Point2::new(
            texcoord.x / self.cur_texture_dim.0 as f64,
            // TODO: t coordinate seems to be wrong for mirrored textures
            1.0 - texcoord.y / self.cur_texture_dim.1 as f64,
        );
    }
    fn color(&mut self, color: Point3<f32>) {
        self.next_color = color;
    }
    fn vertex(&mut self, v: Point3<f64>) {
        self.data.vertices.push(Vertex {
            position: [v.x as f32, v.y as f32, v.z as f32],
            texcoord: [self.next_texcoord.x as f32, self.next_texcoord.y as f32],
            color: [self.next_color.x, self.next_color.y, self.next_color.z],
        });
        self.cur_mesh_range.vertex_range.end += 1;
    }
}
