use std::ops::Range;

/// Builder for a triangle index buffer.
///
/// Consumes NDS primitive groups and produces a list of triangle
/// indices (suitable for GL_TRIANGLES). The builder is only concerned
/// with the topology and doesn't need to know anything about the
/// vertices (like their position, etc.).
#[derive(Debug, Clone)]
pub struct IndexBuilder {
    pub indices: Vec<u16>,
    cur_prim_type: u32,
    cur_prim_range: Range<u16>,
}

impl IndexBuilder {
    pub fn new() -> IndexBuilder {
        IndexBuilder {
            indices: vec![],
            cur_prim_type: 0,
            cur_prim_range: 0..0,
        }
    }

    /// Begin new primitive group. Automatically closes any
    /// previous primitive group.
    pub fn begin(&mut self, prim_type: u32) {
        self.cur_prim_type = prim_type;
        let end = self.cur_prim_range.end;
        self.cur_prim_range = end .. end;
    }

    /// End current primitive group and flush index data to `indices`.
    pub fn end(&mut self) {
        let r = self.cur_prim_range.clone();
        // TODO: check the size of cur_prim_range for validity
        // (eg. for seperate triangles, must be a multiple of 3, etc.)
        match self.cur_prim_type {
            0 => {
                // Seperate triangles
                let mut i = r.start;
                while i != r.end {
                    self.indices.extend_from_slice(&[
                        i, i+1, i+2,
                    ]);
                    i += 3;
                }
            }
            1 => {
                // Seperate quads
                let mut i = r.start;
                while i != r.end {
                    self.indices.extend_from_slice(&[
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
                    self.indices.extend_from_slice(&[
                        i, i+1, i+2,
                    ]);
                    i += 3;
                }
                while i != r.end {
                    self.indices.extend_from_slice(&[
                        i, i-1, i-2,
                    ]);
                    i += 1;
                    if i == r.end { break; }
                    self.indices.extend_from_slice(&[
                        i, i-2, i-1,
                    ]);
                    i += 1;
                }
            }
            3 => {
                // Quad strip
                let mut i = r.start;
                if i != r.end {
                    self.indices.extend_from_slice(&[
                        i, i+1, i+2,
                        i+2, i+1, i+3,
                    ]);
                    i += 4;
                }
                while i != r.end {
                    self.indices.extend_from_slice(&[
                        i-2, i-1, i,
                        i, i-1, i+1,
                    ]);
                    i += 2;
                }
            }
            _ => unreachable!(),
        }
    }

    /// Indicate that a new vertex was added to the primitive group.
    pub fn vertex(&mut self) {
        self.cur_prim_range.end += 1;
    }
}
