use cgmath::Point2;
use cgmath::Point3;
use cgmath::vec3;
use errors::Result;
use util::bits::BitField;
use util::fixed::fix16;
use util::fixed::fix32;
use util::view::View;

pub trait Sink {
    /// Load a matrix from the stack to the current matrix.
    fn restore(&mut self, idx: u32);

    /// Precompose the current matrix with a scaling.
    fn scale(&mut self, scale: (f64, f64, f64));

    /// Begin a primitive group (eg. triangles, quads, etc).
    fn begin(&mut self, prim_type: u32);

    /// End the current primitive group.
    fn end(&mut self);

    /// Send a vertex with the given position.
    ///
    /// Note that the vertex is _untransformed_. It must be multiplied
    /// by the current matrix to get its final position. The implementer
    /// is responsible for tracking the current matrix and doing the
    /// transformation.
    fn vertex(&mut self, position: Point3<f64>);

    /// Set the texture coordinate for the next vertex.
    ///
    /// Texture coordinate on the DS are measured in texels. The top-left
    /// corner of an image is (0,0) and the bottom-right is (w,h), where
    /// w and h are the width and height of the image.
    fn texcoord(&mut self, tc: Point2<f64>);

    /// Set the vertex color for the next vertex.
    fn color(&mut self, color: Point3<f32>);
}

pub fn run_commands<S: Sink>(cmds: &[u8], sink: &mut S) -> Result<()> {
    let mut state = GpuInterpreterState::new();
    state.run_commands(cmds, sink)
}

pub struct GpuInterpreterState {
    pub vertex: Point3<f64>,
}

impl GpuInterpreterState {
    pub fn new() -> GpuInterpreterState {
        GpuInterpreterState {
            vertex: Point3::new(0.0, 0.0, 0.0),
        }
    }

    pub fn run_commands<S: Sink>(&mut self, mut cmds: &[u8], sink: &mut S) -> Result<()> {
        let mut fifo = &cmds[0..0];
        while cmds.len() != 0 {
            if fifo.len() == 0 {
                fifo = &cmds[0..4];
                cmds = &cmds[4..];
            }
            let opcode = fifo[0];
            fifo = &fifo[1..];
            let size = cmd_size(opcode)?;
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
                let id = params.get(0);
                sink.restore(id);
            }
            0x1b => {
                // MTX_SCALE - Multiply Current Matrix by Scale Matrix
                let sx = fix32(params.get(0), 1, 19, 12);
                let sy = fix32(params.get(1), 1, 19, 12);
                let sz = fix32(params.get(2), 1, 19, 12);
                sink.scale((sx, sy, sz));
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
                let texcoord = Point2::new(s,t);
                sink.texcoord(texcoord);
            }
            0x20 => {
                // COLOR - Set Vertex Color
                let p = params.get(0);
                let r = p.bits(0,5) as f32 / 31.0;
                let g = p.bits(5,10) as f32 / 31.0;
                let b = p.bits(10,15) as f32 / 31.0;
                let color = Point3::new(r,g,b);
                sink.color(color);
            }
            0x21 => {
                // NORMAL - Set Normal Vector
                // Not implemented; just ignore it for now.
            }
            _ => {
                warn!("unknown opcode {:#x}", opcode);
            }
        }
    }

    fn push_vertex<S: Sink>(&mut self, sink: &mut S, vertex: Point3<f64>) {
        self.vertex = vertex;
        sink.vertex(vertex);
    }
}


fn cmd_size(opcode: u8) -> Result<usize> {
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
       bail!("unknown geometry opcode: {:#x}", opcode);
    }
    Ok(SIZES[opcode] as usize)
}
