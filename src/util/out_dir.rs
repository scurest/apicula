use std::io::ErrorKind;
use std::path::PathBuf;
use std::fs;
use errors::Result;

/// Directory for putting output files in. Will be created lazily when the first
/// file is created.
pub struct OutDir {
    path: PathBuf,
    created: bool,
}

impl OutDir {
    pub fn new(path: PathBuf) -> Result<OutDir> {
        Ok(OutDir { path, created: false })
    }

    pub fn create_file(&mut self, filename: &str) -> Result<fs::File> {
        if !self.created {
            match fs::create_dir(&self.path) {
                Ok(()) => (),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => (),
                Err(e) => Err(e)?,
            }
            self.created = true;
        }
        Ok(fs::File::create(self.path.join(filename))?)
    }

}
