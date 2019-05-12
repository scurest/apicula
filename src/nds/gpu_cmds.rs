//! NDS GPU command parsing.
//!
//! A `CmdParser` allows consumers to iterate over NDS GPU commands stored in
//! a buffer. This modules doesn't actually do anything with the commands, it
//! just knows how they're stored in memory and provides them in a more easily-
//! consumable form.
//!
//! See the [GBATEK documentation](http://problemkaputt.de/gbatek.htm#ds3dvideo)
//! for a reference on the DS's GPU.

use cgmath::{vec3, Point2, Point3, Vector3};
use errors::Result;
use util::bits::BitField;
use util::fixed::fix16;
use util::fixed::fix32;
use util::view::View;

/// DS GPU command.
pub enum GpuCmd {
    /// Do nothing.
    Nop,

    /// Load the matrix from stack slot `idx` to the current matrix.
    Restore { idx: u32 },

    /// Precompose the current matrix with a scaling.
    Scale { scale: (f64, f64, f64) },

    /// Begin a new primitive group of the given type (tris, quads, etc).
    Begin { prim_type: u32 },

    /// End the current primitive group.
    End,

    /// Send a vertex with the given position.
    ///
    /// Note that the position is _untransformed_. It must be multiplied
    /// by the current matrix to get its final position. The implementer
    /// is responsible for tracking the current matrix and doing the
    /// transformation.
    Vertex { position: Point3<f64> },

    /// Set the texture coordinate for subsequent vertices.
    ///
    /// Texture coordinate on the DS are measured in texels. The top-left
    /// corner of an image is (0,0) and the bottom-right is (w,h), where
    /// w and h are the width and height of the image.
    TexCoord { texcoord: Point2<f64> },

    /// Set the color for subsequent vertices.
    Color { color: Point3<f32> },

    /// Set the normal vector for subsequent vertices.
    Normal { normal: Vector3<f64> },
}

/// Parses the memory representation of GPU commands, yielding them as
/// an iterator.
///
/// A GPU command consists of a one-byte opcode and a sequence of 32-words
/// for parameters. The commands are packed in memory as follows: four
/// commands are packed as a sequence of words
///
/// 1. the four opcodes into the first four bytes
/// 2. then the parameters for the first command, then the parameters for
///    the second command, then the third, then the fourth.
pub struct CmdParser<'a> {
    /// The unprocessed opcodes (never more than four at one time).
    pub opcode_fifo: &'a [u8],

    /// The buffer, starting with the parameters for the next opcode in
    /// `opcode_fifo`, or starting with the next group of opcodes if that's
    /// empty.
    pub buf: &'a [u8],

    /// The last vertex position sent; this is needed for the relative GPU
    /// commands that specify a vertex position by a displacement from the
    /// last one.
    pub vertex: Point3<f64>,

    /// Whether we're done. Always set after an error.
    done: bool,
}

impl<'a> CmdParser<'a> {
    pub fn new(cmds: &[u8]) -> CmdParser {
        CmdParser {
            opcode_fifo: &cmds[0..0],
            buf: cmds,
            vertex: Point3::new(0.0, 0.0, 0.0),
            done: false,
        }
    }
}

impl<'a> Iterator for CmdParser<'a> {
    type Item = Result<GpuCmd>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        // Fill up the opcode FIFO if it has run out.
        if self.opcode_fifo.len() == 0 {
            if self.buf.len() == 0 {
                // Finished successfully.
                self.done = true;
                return None;
            }

            if self.buf.len() < 4 {
                self.done = true;
                return Some(Err("GPU has too few opcodes".into()));
            }

            self.opcode_fifo = &self.buf[0..4];
            self.buf = &self.buf[4..];
        }

        let opcode = self.opcode_fifo[0];
        self.opcode_fifo = &self.opcode_fifo[1..];

        let num_params = match num_params(opcode) {
            Ok(count) => count,
            Err(e) => {
                self.done = true;
                return Some(Err(e));
            }
        };
        let num_param_bytes = 4 * num_params;
        if self.buf.len() < num_param_bytes {
            self.done = true;
            return Some(Err("buffer too short for GPU opcode parameters".into()));
        }
        let params = View::from_buf(&self.buf[0..num_param_bytes]);
        self.buf = &self.buf[num_param_bytes..];

        Some(parse(self, opcode, params))
    }
}

fn parse(state: &mut CmdParser, opcode: u8, params: View<u32>) -> Result<GpuCmd> {
    Ok(match opcode {
        // NOP
        0x00 => GpuCmd::Nop,

        // MTX_RESTORE - Restore Current Matrix from Stack
        0x14 => {
            let idx = params.nth(0) & 31;
            GpuCmd::Restore { idx }
        }

        // MTX_SCALE - Multiply Current Matrix by Scale Matrix
        0x1b => {
            let sx = fix32(params.nth(0), 1, 19, 12);
            let sy = fix32(params.nth(1), 1, 19, 12);
            let sz = fix32(params.nth(2), 1, 19, 12);
            GpuCmd::Scale {
                scale: (sx, sy, sz),
            }
        }

        // BEGIN_VTXS - Start of Vertex List
        0x40 => {
            let prim_type = params.nth(0) & 3;
            GpuCmd::Begin { prim_type }
        }

        // END_VTXS - End of Vertex List
        0x41 => GpuCmd::End,

        // VTX_16 - Set Vertex XYZ Coordinates
        0x23 => {
            let p0 = params.nth(0);
            let p1 = params.nth(1);
            let x = fix16(p0.bits(0, 16) as u16, 1, 3, 12);
            let y = fix16(p0.bits(16, 32) as u16, 1, 3, 12);
            let z = fix16(p1.bits(0, 16) as u16, 1, 3, 12);
            let position = Point3::new(x, y, z);
            state.vertex = position;
            GpuCmd::Vertex { position }
        }

        // VTX_10 - Set Vertex XYZ Coordinates
        0x24 => {
            let p = params.nth(0);
            let x = fix16(p.bits(0, 10) as u16, 1, 3, 6);
            let y = fix16(p.bits(10, 20) as u16, 1, 3, 6);
            let z = fix16(p.bits(20, 30) as u16, 1, 3, 6);
            let position = Point3::new(x, y, z);
            state.vertex = position;
            GpuCmd::Vertex { position }
        }

        // VTX_XY - Set Vertex XY Coordinates
        0x25 => {
            let p = params.nth(0);
            let x = fix16(p.bits(0, 16) as u16, 1, 3, 12);
            let y = fix16(p.bits(16, 32) as u16, 1, 3, 12);
            let position = Point3::new(x, y, state.vertex.z);
            state.vertex = position;
            GpuCmd::Vertex { position }
        }

        // VTX_XZ - Set Vertex XZ Coordinates
        0x26 => {
            let p = params.nth(0);
            let x = fix16(p.bits(0, 16) as u16, 1, 3, 12);
            let z = fix16(p.bits(16, 32) as u16, 1, 3, 12);
            let position = Point3::new(x, state.vertex.y, z);
            state.vertex = position;
            GpuCmd::Vertex { position }
        }

        // VTX_YZ - Set Vertex YZ Coordinates
        0x27 => {
            let p = params.nth(0);
            let y = fix16(p.bits(0, 16) as u16, 1, 3, 12);
            let z = fix16(p.bits(16, 32) as u16, 1, 3, 12);
            let position = Point3::new(state.vertex.x, y, z);
            state.vertex = position;
            GpuCmd::Vertex { position }
        }

        // VTX_DIFF - Set Relative Vertex Coordinates
        0x28 => {
            let p = params.nth(0);
            // Differences are 10-bit numbers, scaled by 1/2^3 to put them
            // in the same 1,3,12 format as the others VTX commands.
            let scale = (0.5f64).powi(3);
            let dx = scale * fix16(p.bits(0, 10) as u16, 1, 0, 9);
            let dy = scale * fix16(p.bits(10, 20) as u16, 1, 0, 9);
            let dz = scale * fix16(p.bits(20, 30) as u16, 1, 0, 9);
            let position = state.vertex + vec3(dx, dy, dz);
            state.vertex = position;
            GpuCmd::Vertex { position }
        }

        // TEXCOORD - Set Texture Coordinates
        0x22 => {
            let p = params.nth(0);
            let s = fix16(p.bits(0, 16) as u16, 1, 11, 4);
            let t = fix16(p.bits(16, 32) as u16, 1, 11, 4);
            let texcoord = Point2::new(s, t);
            GpuCmd::TexCoord { texcoord }
        }

        // COLOR - Set Vertex Color
        0x20 => {
            let p = params.nth(0);
            let r = p.bits(0, 5) as f32 / 31.0;
            let g = p.bits(5, 10) as f32 / 31.0;
            let b = p.bits(10, 15) as f32 / 31.0;
            let color = Point3::new(r, g, b);
            GpuCmd::Color { color }
        }

        // NORMAL - Set Normal Vector
        0x21 => {
            let p = params.nth(0);
            let x = fix32(p.bits(0, 10), 1, 0, 9);
            let y = fix32(p.bits(10, 20), 1, 0, 9);
            let z = fix32(p.bits(20, 30), 1, 0, 9);
            let normal = vec3(x, y, z);
            GpuCmd::Normal { normal }
        }

        _ => {
            bail!("unimplented GPU ocpode: {:#x}", opcode);
        }
    })
}

/// Number of u32 parameters `opcode` takes.
fn num_params(opcode: u8) -> Result<usize> {
    static SIZES: [i8; 66] = [
        0, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 1, 0, 1, 1, 1, 0, 16, 12,
        16, 12, 9, 3, 3, -1, -1, -1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1, 1, 1, 1,
        1, 1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 1, 0,
    ];
    let opcode = opcode as usize;
    if opcode >= SIZES.len() || SIZES[opcode] == -1 {
        bail!("unknown GPU opcode: {:#x}", opcode);
    }
    Ok(SIZES[opcode] as usize)
}
