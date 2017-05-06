//! File reading and collecting.
//!
//! This module reads input files (from the command-line) into an owning
//! `BufferHolder`, and then tries to parse each as a Nitro file. The resulting
//! Nitro files are sorted by kind into vectors for models, animations, etc.
//! The resulting `FileHolder` is all the input data for this invocation of
//! the program.

use errors::Result;
use nitro::container::DataFile;
use nitro::container::Container;
use nitro::jnt::Animation;
use nitro::mdl::Model;
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
    Container(Container<'a>),
    DataFile(DataFile<'a>),
}

fn read_file(buf: &[u8]) -> Result<File> {
    let cur = Cur::new(buf);
    let stamp = cur.clone().next_n_u8s(4)?;
    match stamp {
        b"BMD0" | b"BTX0" | b"BCA0" => Ok(File::Container(Container::read(cur)?)),
        b"MDL0" | b"TEX0" | b"JNT0" => Ok(File::DataFile(DataFile::read(cur)?)),
        _ => bail!("unknown file type"),
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
            File::DataFile(data_file) => {
                match data_file {
                    DataFile::Mdl(mut mdl) => self.models.append(&mut mdl.models),
                    DataFile::Tex(tex) => self.texs.push(tex),
                    DataFile::Jnt(mut jnt) => self.animations.append(&mut jnt.animations),
                }
            }
            File::Container(cont) => {
                let valid_data_files = cont.data_files.into_iter()
                    .filter_map(|res| res.ok());
                for data_file in valid_data_files {
                    self.add_file(File::DataFile(data_file));
                }
            }
        }
    }
}
