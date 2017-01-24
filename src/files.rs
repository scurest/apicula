//! File reading and collecting.
//!
//! This module reads input files (from the command-line) into an owning
//! `BufferHolder`, and then tries to parse each as a Nitro file. The resulting
//! Nitro files are sorted by kind into vectors for models, animations, etc.
//! The resulting `FileHolder` is all the input data for this invocation of
//! the program.

use errors::Result;
use nitro::bca::Bca;
use nitro::bca::read_bca;
use nitro::bmd::Bmd;
use nitro::bmd::read_bmd;
use nitro::jnt::Animation;
use nitro::jnt::Jnt;
use nitro::jnt::read_jnt;
use nitro::mdl::Mdl;
use nitro::mdl::Model;
use nitro::mdl::read_mdl;
use nitro::tex::read_tex;
use nitro::tex::Tex;
use std::fs;
use std::io::Read;
use std::path::Path;
use util::cur::Cur;

pub struct Buffer {
    pub name: String,
    pub bytes: Vec<u8>,
}

pub struct BufferHolder {
    buffers: Vec<Buffer>,
}

impl BufferHolder {
    pub fn read_files<Iter, T>(paths: Iter) -> Result<BufferHolder>
    where Iter: Iterator<Item=T>, T: AsRef<Path> {
        let buffers = paths
            .map(|path| {
                let path = path.as_ref();
                let mut f = fs::File::open(path)?;
                let mut bytes = vec![];
                f.read_to_end(&mut bytes)?;

                let name = path.to_string_lossy().into_owned();

                Ok(Buffer { name: name, bytes: bytes })
            })
            .collect::<Result<_>>()?;
        Ok(BufferHolder { buffers: buffers })
    }
}

pub struct FileHolder<'a> {
    pub models: Vec<Model<'a>>,
    pub animations: Vec<Animation<'a>>,
    pub texs: Vec<Tex<'a>>,
}

enum File<'a> {
    Bmd(Bmd<'a>),
    Mdl(Mdl<'a>),
    Tex(Tex<'a>),
    Bca(Bca<'a>),
    Jnt(Jnt<'a>),
}

fn read_file(buf: &[u8]) -> Result<File> {
    let cur = Cur::new(buf);
    let stamp = cur.clone().next_n_u8s(4)?;
    match stamp {
        b"BMD0" => Ok(File::Bmd(read_bmd(cur)?)),
        b"MDL0" => Ok(File::Mdl(read_mdl(cur)?)),
        b"TEX0" => Ok(File::Tex(read_tex(cur)?)),
        b"BCA0" => Ok(File::Bca(read_bca(cur)?)),
        b"JNT0" => Ok(File::Jnt(read_jnt(cur)?)),
        _ => Err("unknown file type".into())
    }
}

impl<'a> FileHolder<'a> {
    pub fn from_buffers(buf_holder: &BufferHolder) -> FileHolder {
        let mut file_holder = FileHolder {
            models: vec![],
            animations: vec![],
            texs: vec![],
        };

        for buffer in &buf_holder.buffers {
            let res = read_file(&buffer.bytes[..]);
            match res {
                Ok(file) => file_holder.add_file(file),
                Err(e) => {
                    error!("error reading buffer {}: {:?}", buffer.name, e);
                }
            }
        }

        file_holder
    }

    fn add_file(&mut self, file: File<'a>) {
        match file {
            File::Mdl(mut mdl) => self.models.append(&mut mdl.models),
            File::Jnt(mut jnt) => self.animations.append(&mut jnt.animations),
            File::Tex(tex) => self.texs.push(tex),
            File::Bmd(bmd) => {
                for mdl in bmd.mdls {
                    self.add_file(File::Mdl(mdl));
                }
                for tex in bmd.texs {
                    self.add_file(File::Tex(tex));
                }
            }
            File::Bca(bca) => {
                for jnt in bca.jnts {
                    self.add_file(File::Jnt(jnt));
                }
            }
        }
    }
}
