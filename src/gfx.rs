use cgmath::Matrix4;
use cgmath::Point2;
use cgmath::Point3;
use cgmath::Transform;
use cgmath::vec3;
use cgmath::vec4;
use errors::Result;
use util::bits::BitField;
use util::fixed::fix16;
use util::fixed::fix32;
use util::view::View;

pub trait Sink {
    fn begin(&mut self, prim_type: u32);
    fn end(&mut self);
    fn vertex(&mut self, v: Point3<f64>);
    fn texcoord(&mut self, tc: Point2<f64>);
    fn color(&mut self, color: Point3<f32>);
}

pub struct GfxState {
    pub vertex: Point3<f64>,
    pub cur_mat: Matrix4<f64>,
    pub mat_stack: Vec<Matrix4<f64>>,
    pub texture_mat: Matrix4<f64>,
}

impl GfxState {
    pub fn new() -> GfxState {
        GfxState {
            vertex: Point3::new(0.0, 0.0, 0.0),
            cur_mat: Matrix4::from_scale(1.0),
            mat_stack: vec![Matrix4::from_scale(1.0); 32],
            texture_mat: Matrix4::from_scale(1.0),
        }
    }

    pub fn restore(&mut self, id: u32) {
        self.cur_mat = self.mat_stack[id as usize];
    }

    pub fn run_commands<S: Sink>(&mut self, sink: &mut S, mut cmds: &[u8]) -> Result<()> {
        let mut fifo = &cmds[0..0];
        while cmds.len() != 0 {
            if fifo.len() == 0 {
                fifo = &cmds[0..4];
                cmds = &cmds[4..];
            }
            let opcode = fifo[0];
            fifo = &fifo[1..];
            let size = gfx_cmd_size(opcode)?;
            let params = View::from_buf(&cmds[0..4*size]);
            cmds = &cmds[4*size..];
            self.run_command(sink, opcode, params);
        }
        Ok(())
    }

    pub fn run_command<S: Sink>(&mut self, sink: &mut S, opcode: u8, params: View<u32>) {
        match opcode {
            0x00 => {
                // NOP
            }
            0x14 => {
                // MTX_RESTORE - Restore Current Matrix from Stack
                let restore_id = params.get(0);
                self.restore(restore_id);
            }
            0x1b => {
                // MTX_SCALE - Multiply Current Matrix by Scale Matrix
                let sx = fix32(params.get(0), 1, 19, 12);
                let sy = fix32(params.get(1), 1, 19, 12);
                let sz = fix32(params.get(2), 1, 19, 12);
                self.cur_mat = self.cur_mat * Matrix4::from_nonuniform_scale(sx, sy, sz);
            }
            0x40 => {
                // BEGIN_VTXS - Start of Vertex List
                let prim_type = params.get(0);
                sink.begin(prim_type);
            }
            0x41 => {
                // END_VTXS - End of Vertex List
                sink.end();
            }
            0x23 => {
                // VTX_16 - Set Vertex XYZ Coordinates
                let p0 = params.get(0);
                let p1 = params.get(1);
                let x = fix16(p0.bits(0,16) as u16, 1, 3, 12);
                let y = fix16(p0.bits(16,32) as u16, 1, 3, 12);
                let z = fix16(p1.bits(0,16) as u16, 1, 3, 12);
                let v = Point3::new(x, y, z);
                self.push_vertex(sink, v);
            }
            0x24 => {
                // VTX_10 - Set Vertex XYZ Coordinates
                let p = params.get(0);
                let x = fix16(p.bits(0,10) as u16, 1, 3, 6);
                let y = fix16(p.bits(10,20) as u16, 1, 3, 6);
                let z = fix16(p.bits(20,30) as u16, 1, 3, 6);
                let v = Point3::new(x, y, z);
                self.push_vertex(sink, v);
            }
            0x25 => {
                // VTX_XY - Set Vertex XY Coordinates
                let p = params.get(0);
                let x = fix16(p.bits(0,16) as u16, 1, 3, 12);
                let y = fix16(p.bits(16,32) as u16, 1, 3, 12);
                let v = Point3::new(x, y, self.vertex.z);
                self.push_vertex(sink, v);
            }
            0x26 => {
                // VTX_XZ - Set Vertex XZ Coordinates
                let p = params.get(0);
                let x = fix16(p.bits(0,16) as u16, 1, 3, 12);
                let z = fix16(p.bits(16,32) as u16, 1, 3, 12);
                let v = Point3::new(x, self.vertex.y, z);
                self.push_vertex(sink, v);
            }
            0x27 => {
                // VTX_YZ - Set Vertex YZ Coordinates
                let p = params.get(0);
                let y = fix16(p.bits(0,16) as u16, 1, 3, 12);
                let z = fix16(p.bits(16,32) as u16, 1, 3, 12);
                let v = Point3::new(self.vertex.x, y, z);
                self.push_vertex(sink, v);
            }
            0x28 => {
                // VTX_DIFF - Set Relative Vertex Coordinates
                let p = params.get(0);
                // Differences are 10-bit numbers, scaled by 1/2^3 to put them
                // in the same 1,3,12 format as the others VTX commands.
                let scale = 0.5f64.powi(3);
                let dx = scale * fix16(p.bits(0,10) as u16, 1, 0, 9);
                let dy = scale * fix16(p.bits(10,20) as u16, 1, 0, 9);
                let dz = scale * fix16(p.bits(20,30) as u16, 1, 0, 9);
                let v = self.vertex + vec3(dx, dy, dz);
                self.push_vertex(sink, v);
            }
            0x22 => {
                // TEXCOORD - Set Texture Coordinates
                let p = params.get(0);
                let s = fix16(p.bits(0,16) as u16, 1, 11, 4);
                let t = fix16(p.bits(16,32) as u16, 1, 11, 4);
                let tc = self.texture_mat * vec4(s,t,0.0,0.0);
                let texcoord = Point2::new(tc.x, tc.y);
                sink.texcoord(texcoord);
            }
            0x20 => {
                let p = params.get(0);
                let r = p.bits(0,5) as f32 / 31.0;
                let g = p.bits(5,10) as f32 / 31.0;
                let b = p.bits(10,15) as f32 / 31.0;
                let color = Point3::new(r,g,b);
                sink.color(color);
            }
            0x21 => {
                // normal
            }
            _ => { panic!("unknown opcode {:#x}", opcode); }
        }
    }

    fn push_vertex<S: Sink>(&mut self, sink: &mut S, vertex: Point3<f64>) {
        self.vertex = vertex;
        sink.vertex(self.cur_mat.transform_point(vertex));
    }
}


fn gfx_cmd_size(opcode: u8) -> Result<usize> {
    static SIZES: [i8; 66] = [
         0, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1,  1,  0,  1,  1,  1,  0,
        16, 12, 16, 12,  9,  3,  3, -1, -1, -1,  1,
         1,  1,  2,  1,  1,  1,  1,  1,  1,  1,  1,
        -1, -1, -1, -1,  1,  1,  1,  1,  1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1,  1,  0,
    ];
    let opcode = opcode as usize;
    if opcode >= SIZES.len() || SIZES[opcode] == -1 {
        return Err(format!("unknown geometry opcode: {:#x}", opcode).into());
    }
    Ok(SIZES[opcode] as usize)
}
