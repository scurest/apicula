use clap::ArgMatches;
use std::path::{PathBuf, Path};
use std::collections::HashMap;
use nitro::{Name, Model, Texture, Palette, Animation};
use errors::Result;
use util::cur::Cur;

type TextureId = usize;
type PaletteId = usize;

#[derive(Default)]
pub struct Database {
    pub file_paths: Vec<PathBuf>,

    pub models: Vec<Model>,
    pub textures: Vec<Texture>,
    pub palettes: Vec<Palette>,
    pub animations: Vec<Animation>,

    pub textures_by_name: HashMap<Name, TextureId>,
    pub palettes_by_name: HashMap<Name, PaletteId>,
}

impl Database {
    pub fn from_arg_matches(matches: &ArgMatches) -> Result<Database> {
        let file_paths: Vec<PathBuf> =
            matches
            .values_of_os("INPUT").unwrap()
            .map(PathBuf::from)
            .collect();
        Database::build(file_paths)
    }

    pub fn build(file_paths: Vec<PathBuf>) -> Result<Database> {
        use std::default::Default;

        let mut db: Database = Default::default();
        db.file_paths = file_paths;

        debug!("Building database...");

        for path in &db.file_paths {
            debug!("Processing {:?}...", path);

            // Hard-fail if we can't open the path. We don't expect the caller
            // to know which files are valid Nitro files but we expect them to
            // give us files we can actually open.
            let buf = read_file(path)?;

            use nitro::container::read_container;
            match read_container(Cur::new(&buf)) {
                Ok(cont) => {
                    db.models.extend(cont.models.into_iter());
                    db.textures.extend(cont.textures.into_iter());
                    db.palettes.extend(cont.palettes.into_iter());
                    db.animations.extend(cont.animations.into_iter());
                }
                Err(e) => {
                    error!("error in file {}: {}", path.to_string_lossy(), e);
                }
            }
        }

        db.build_by_name_maps();

        Ok(db)
    }

    /// Fill out `textures_by_name` and `palettes_by_name`.
    fn build_by_name_maps(&mut self) {
        use std::collections::hash_map::Entry::*;

        let mut name_clash = false;
        for (id, texture) in self.textures.iter().enumerate() {
            match self.textures_by_name.entry(texture.name) {
                Vacant(ve) => { ve.insert(id); },
                Occupied(_) => {
                    warn!("multiple textures have the name {}", texture.name);
                    name_clash = true;
                }
            }
        }

        for (id, palette) in self.palettes.iter().enumerate() {
            match self.palettes_by_name.entry(palette.name) {
                Vacant(ve) => { ve.insert(id); },
                Occupied(_) => {
                    warn!("multiple palettes have the name {}", palette.name);
                    name_clash = true;
                }
            }
        }

        if name_clash {
            warn!("since there were name clashes, some textures might be wrong");
        }
    }

    pub fn print_status(&self) {
        let num_models = self.models.len();
        let num_textures = self.textures.len();
        let num_palettes = self.palettes.len();
        let num_animations = self.animations.len();

        let plural = |x| if x != 1 { "s" } else { "" };
        println!("\nGot {} model{}, {} texture{}, {} palette{}, {} animation{}.\n",
            num_models, plural(num_models), num_textures, plural(num_textures),
            num_palettes, plural(num_palettes), num_animations, plural(num_animations),
        );
    }

}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    use std::{fs::File, io::Read};
    let mut f = File::open(&path)?;
    let mut b: Vec<u8> = vec![];
    f.read_to_end(&mut b)?;
    Ok(b)
}
