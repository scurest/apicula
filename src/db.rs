use clap::ArgMatches;
use std::path::PathBuf;
use std::fs;
use std::collections::HashMap;
use nitro::{Name, Model, Texture, Palette, Animation, Pattern, Container};
use errors::Result;
use util::cur::Cur;

pub type FileId = usize;
pub type ModelId = usize;
pub type TextureId = usize;
pub type PaletteId = usize;
pub type AnimationId = usize;
pub type PatternId = usize;

#[derive(Default)]
pub struct Database {
    pub file_paths: Vec<PathBuf>,

    pub models: Vec<Model>,
    pub textures: Vec<Texture>,
    pub palettes: Vec<Palette>,
    pub animations: Vec<Animation>,
    pub patterns: Vec<Pattern>,

    pub models_found_in: Vec<FileId>,
    pub textures_found_in: Vec<FileId>,
    pub palettes_found_in: Vec<FileId>,
    pub animations_found_in: Vec<FileId>,
    pub patterns_found_in: Vec<FileId>,

    pub textures_by_name: HashMap<Name, Vec<TextureId>>,
    pub palettes_by_name: HashMap<Name, Vec<PaletteId>>,
}

impl Database {
    pub fn from_arg_matches(matches: &ArgMatches) -> Result<Database> {
        let user_paths =
            matches.values_of_os("INPUT").unwrap()
            .map(PathBuf::from);
        let file_paths = expand_directories(user_paths);

        use std::default::Default;
        let mut db: Database = Default::default();
        db.build(file_paths)?;
        Ok(db)
    }

    pub fn print_status(&self) {
        let num_models = self.models.len();
        let num_textures = self.textures.len();
        let num_palettes = self.palettes.len();
        let num_animations = self.animations.len();
        let num_patterns = self.patterns.len();

        let plural = |x| if x != 1 { "s" } else { "" };
        println!(
            "Got {} model{}, {} texture{}, {} palette{}, {} animation{}, {} pattern animation{}.",
            num_models, plural(num_models),
            num_textures, plural(num_textures),
            num_palettes, plural(num_palettes),
            num_animations, plural(num_animations),
            num_patterns, plural(num_patterns),
        );
    }

    fn build(&mut self, file_paths: Vec<PathBuf>) -> Result<()> {
        self.file_paths = file_paths;

        debug!("Building database...");

        for file_id in 0..self.file_paths.len() {
            debug!("Processing {:?}...", self.file_paths[file_id]);

            let buf = match std::fs::read(&self.file_paths[file_id]) {
                Ok(buf) => buf,
                Err(e) =>{
                    error!("file-system error reading {}: {}",
                        self.file_paths[file_id].to_string_lossy(), e,
                    );
                    continue;
                }
            };

            use nitro::container::read_container;
            match read_container(Cur::new(&buf)) {
                Ok(cont) => {
                    self.add_container(file_id, cont);
                }
                Err(e) => {
                    error!("couldn't parse as a Nitro file: {}: {}",
                        self.file_paths[file_id].to_string_lossy(), e);
                }
            }
        }

        self.build_by_name_maps();
        Ok(())
    }

    fn add_container(&mut self, file_id: FileId, cont: Container) {
        use std::iter::repeat;

        macro_rules! move_from_cont {
            ($kind:ident, $kind_found_in:ident) => {
                let num = cont.$kind.len();
                self.$kind.extend(cont.$kind.into_iter());
                self.$kind_found_in.extend(repeat(file_id).take(num));
            };
        }

        move_from_cont!(models, models_found_in);
        move_from_cont!(textures, textures_found_in);
        move_from_cont!(palettes, palettes_found_in);
        move_from_cont!(animations, animations_found_in);
        move_from_cont!(patterns, patterns_found_in);
    }

    /// Fill out `textures_by_name` and `palettes_by_name`.
    fn build_by_name_maps(&mut self) {
        for (id, texture) in self.textures.iter().enumerate() {
            self.textures_by_name.entry(texture.name)
                .or_insert(vec![])
                .push(id);
        }

        for (id, palette) in self.palettes.iter().enumerate() {
            self.palettes_by_name.entry(palette.name)
                .or_insert(vec![])
                .push(id);
        }
    }
}

/// Collects the user's provided paths, expanding any that are directories into
/// their children (only expand once, not recursively).
fn expand_directories<I: Iterator<Item=PathBuf>>(paths: I) -> Vec<PathBuf> {
    let mut file_paths = vec![];
    for path in paths {
        if path.is_dir() {
            let start_idx = file_paths.len();

            let entries = match fs::read_dir(path) {
                Ok(entries) => entries,
                Err(e) => {
                    warn!("error reading directory: {}", e);
                    continue;
                }
            };
            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => continue,
                };
                file_paths.push(entry.path());
            }

            let end_idx = file_paths.len();
            file_paths[start_idx..end_idx].sort();
        } else {
            file_paths.push(path);
        }
    }
    file_paths
}
