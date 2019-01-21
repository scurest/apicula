use json::JsonValue;
use std::io::Write;

pub struct Glb {
    pub buffers: Vec<Buffer>,
    pub gltf: JsonValue,
}

pub struct Buffer {
    pub bytes: Vec<u8>,
    pub alignment: usize,
}

impl Glb {
    pub fn new() -> Glb {
        Glb {
            buffers: vec![],
            gltf: object!(
                "asset" => object!(
                    "version" => "2.0",
                ),
                "bufferViews" => array!(),
                "accessors" => array!(),
            )
        }
    }

    pub fn write<W: Write>(mut self, w: &mut W) -> std::io::Result<()> {
        // Lay all the buffers out into one GLB buffer.

        // Gives the offset to the start of each buffer into the GLB buffer
        let mut buf_offsets = Vec::with_capacity(self.buffers.len());
        let bin_len = {
            let mut l = 0_usize;
            for buffer in &self.buffers {
                // Pad to alignment
                if l % buffer.alignment != 0 {
                    l += buffer.alignment - (l % buffer.alignment);
                }

                buf_offsets.push(l);

                l += buffer.bytes.len();
            }
            if l % 4 != 0 {
                l += 4 - (l % 4);
            }
            l
        };

        // Add the glTF buffer
        self.gltf["buffers"] = array!(
            object!(
                "byteLength" => bin_len,
            )
        );

        // Fixup bufferView references to be the single buffer
        for buf_view in self.gltf["bufferViews"].members_mut() {
            let old_buf_idx = buf_view["buffer"].as_usize().unwrap();
            buf_view["buffer"] = 0.into();
            let offset = if buf_view.has_key("byteOffset") {
                buf_view["byteOffset"].as_usize().unwrap()
            } else {
                0
            };
            let new_offset = offset + buf_offsets[old_buf_idx];
            buf_view["byteOffset"] = new_offset.into();
        }

        // JSON -> String
        let mut s = self.gltf.dump();
        while s.len() % 4 != 0 {
            s.push(' ');
        }

        // Calculate total filesize
        let filesize =
            12 + // GLB Header
            8 + // JSON Chunk Header
            s.len() + // JSON Chunk Data
            8 + // BIN Chunk Header
            bin_len; // BIN Chunk Data

        // Scratch buffer
        let mut scratch = Vec::<u8>::with_capacity(24);

        // GLB Header
        scratch.extend_from_slice(b"glTF");
        scratch.push_u32(2);
        scratch.push_u32(filesize as u32);
        // JSON Chunk Header
        scratch.push_u32(s.len() as u32);
        scratch.extend_from_slice(b"JSON");
        w.write(&scratch)?;
        // JSON Chunk Data
        w.write(s.as_bytes())?;
        // BIN Chunk Header
        scratch.clear();
        scratch.push_u32(bin_len as u32);
        scratch.extend_from_slice(b"BIN\0");
        w.write(&scratch)?;
        // Write all the buffer into the BIN data
        scratch.clear();
        let mut l = 0;
        for buffer in &self.buffers {
            // Pad to alignment
            if l % buffer.alignment != 0 {
                scratch.resize(buffer.alignment - (l % buffer.alignment), 0);
                w.write(&scratch)?;
            }

            w.write(&buffer.bytes)?;

            l += buffer.bytes.len();
        }

        Ok(())
    }
}


pub trait ByteVec {
    fn push_u16(&mut self, x: u16);
    fn push_u32(&mut self, x: u32);
    fn push_f32(&mut self, x: f32);
    fn push_normalized_u8(&mut self, x: f32);
}

impl ByteVec for Vec<u8> {
    fn push_u16(&mut self, x: u16) {
        use std::ptr;
        self.reserve(2);
        let l = self.len();
        unsafe {
            self.set_len(l + 2);
            let p = &mut self[l] as *mut u8 as *mut u16;
            ptr::write_unaligned(p, x.to_le())
        }
    }
    fn push_u32(&mut self, x: u32) {
        use std::ptr;
        self.reserve(4);
        let l = self.len();
        unsafe {
            self.set_len(l + 4);
            let p = &mut self[l] as *mut u8 as *mut u32;
            ptr::write_unaligned(p, x.to_le())
        }
    }
    fn push_f32(&mut self, x: f32) {
        self.push_u32(x.to_bits())
    }
    fn push_normalized_u8(&mut self, x: f32) {
        self.push((x * 255.0).round() as u8);
    }
}

pub trait VecExt<T> {
    fn add(&mut self, x: T) -> usize;
}

impl<T> VecExt<T> for Vec<T> {
    fn add(&mut self, x: T) -> usize {
        self.push(x);
        self.len() - 1
    }
}

impl VecExt<JsonValue> for JsonValue {
    fn add(&mut self, x: JsonValue) -> usize {
        self.push(x).unwrap();
        self.len() - 1
    }
}
