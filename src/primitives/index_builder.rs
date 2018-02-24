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

    fn clear_current_range(&mut self) {
        let end = self.cur_prim_range.end;
        self.cur_prim_range = end .. end;
    }

    /// Begin new primitive group. Automatically closes any
    /// previous primitive group.
    pub fn begin(&mut self, prim_type: u32) {
        self.end();

        self.cur_prim_type = prim_type;
        self.clear_current_range();
    }

    /// End current primitive group and write index data to `indices`.
    pub fn end(&mut self) {
        let start = self.cur_prim_range.start;
        let end = self.cur_prim_range.end;

        if start == end { return; }

        // Indicates if all the indices were used (ie. nothing was left open).
        let complete;

        match self.cur_prim_type {
            0 => {
                // Separate triangles
                //    0      5
                //   / \    / \
                //  1---2  3---4
                let mut i = start;
                while i + 2 < end {
                    self.indices.extend_from_slice(&[
                        i, i+1, i+2,
                    ]);
                    i += 3;
                }
                complete = i == end;
            }

            1 => {
                // Separate quads
                //  0---3  6---5
                //  |   |  |   |
                //  1---2  7---4
                let mut i = start;
                while i + 3 < end {
                    self.indices.extend_from_slice(&[
                        i, i+1, i+2,
                        i+2, i+3, i,
                    ]);
                    i += 4;
                }
                complete = i == end;
            }

            2 => {
                // Triangle strip
                //  0---2---4
                //   \ / \ / \
                //    1---3---5
                let mut i = start;
                let mut odd = false;
                while i + 2 < end {
                    let tri = match odd {
                        false => [i, i+1, i+2],
                        true => [i, i+2, i+1],
                    };
                    self.indices.extend_from_slice(&tri);
                    i += 1;
                    odd = !odd;
                }
                complete = end - start > 2;
            }

            3 => {
                // Quad strip
                //  0---2---4
                //  |   |   |
                //  1---3---5
                let mut i = start;
                while i + 3 < end {
                    self.indices.extend_from_slice(&[
                        i, i+1, i+2,
                        i+2, i+1, i+3,
                    ]);
                    i += 2;
                }
                complete = end - start > 3 && i + 2 == end;
            }

            _ => unreachable!(),
        }

        if !complete {
            warn!("a primitive group was left open, ie. there weren't enough indices \
                   to complete a primitive");
        }

        self.clear_current_range();
    }

    /// Add a new vertex to the current primitive group.
    pub fn vertex(&mut self) {
        self.cur_prim_range.end += 1;
    }
}
