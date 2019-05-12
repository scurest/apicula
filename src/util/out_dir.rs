use errors::Result;
use std::fs;
use std::path::PathBuf;

/// Directory for putting output files in. Will be created lazily when the first
/// file is created.
pub struct OutDir {
    path: PathBuf,
    created: bool,
}

impl OutDir {
    /// Checks that the directory does not already exist. This makes sure we
    /// don't overwrite any existing files.
    pub fn make_ready(path: PathBuf) -> Result<OutDir> {
        if path.exists() {
            bail!(
                "the output directory should be fresh; {} already exists",
                path.to_string_lossy()
            );
        }
        Ok(OutDir {
            path,
            created: false,
        })
    }

    pub fn create_file(&mut self, filename: &str) -> Result<fs::File> {
        if !self.created {
            fs::create_dir(&self.path)?;
            self.created = true;
        }
        Ok(fs::File::create(self.path.join(filename))?)
    }
}
