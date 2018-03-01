use clap::ArgMatches;
use std::path::{PathBuf, Path};
use std::collections::HashMap;
use nitro::{Name, Model, Texture, Palette, Animation, Container};
use errors::Result;
use util::cur::Cur;

pub type FileId = usize;
pub type ModelId = usize;
pub type TextureId = usize;
pub type PaletteId = usize;
pub type MaterialId = usize;

#[derive(Default)]
pub struct Database {
    /// Files provided by the user on the command line.
    pub file_paths: Vec<PathBuf>,

    pub models: Vec<Model>,
    pub textures: Vec<Texture>,
    pub palettes: Vec<Palette>,
    pub animations: Vec<Animation>,

    pub models_found_in: Vec<FileId>,
    pub textures_found_in: Vec<FileId>,
    pub palettes_found_in: Vec<FileId>,
    pub animations_found_in: Vec<FileId>,

    pub material_table: HashMap<(ModelId, MaterialId), ImageDesc>,

    pub textures_by_name: HashMap<Name, Vec<TextureId>>,
    pub palettes_by_name: HashMap<Name, Vec<PaletteId>>,
}

pub enum ImageDesc {
    NoImage,
    Image {
        texture_id: TextureId,
        palette_id: Option<PaletteId>,
    },
    Missing,
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

    pub fn print_status(&self) {
        let num_models = self.models.len();
        let num_textures = self.textures.len();
        let num_palettes = self.palettes.len();
        let num_animations = self.animations.len();

        let plural = |x| if x != 1 { "s" } else { "" };
        println!("Got {} model{}, {} texture{}, {} palette{}, {} animation{}.",
            num_models, plural(num_models), num_textures, plural(num_textures),
            num_palettes, plural(num_palettes), num_animations, plural(num_animations),
        );
    }

    pub fn build(file_paths: Vec<PathBuf>) -> Result<Database> {
        use std::default::Default;

        let mut db: Database = Default::default();
        db.file_paths = file_paths;

        debug!("Building database...");

        for file_id in 0..db.file_paths.len() {
            debug!("Processing {:?}...", db.file_paths[file_id]);

            // Hard-fail if we can't open the path. We don't expect the caller
            // to know which files are valid Nitro files but we expect them to
            // give us files we can actually open.
            let buf = read_file(&db.file_paths[file_id])?;

            use nitro::container::read_container;
            match read_container(Cur::new(&buf)) {
                Ok(cont) => {
                    db.add_container(file_id, cont);
                }
                Err(e) => {
                    error!("error in file {}: {}",
                        db.file_paths[file_id].to_string_lossy(), e);
                }
            }
        }

        db.build_by_name_maps();
        db.build_material_table();

        Ok(db)
    }

    fn add_container(&mut self, file_id: FileId, cont: Container) {
        use std::iter::repeat;

        let num_models = cont.models.len();
        let num_textures = cont.textures.len();
        let num_palettes = cont.palettes.len();
        let num_animations = cont.animations.len();

        // Move the items from the container into the DB, marking which
        // file we found them in as we go.

        self.models.extend(cont.models.into_iter());
        self.models_found_in.extend(repeat(file_id).take(num_models));

        self.textures.extend(cont.textures.into_iter());
        self.textures_found_in.extend(repeat(file_id).take(num_textures));

        self.palettes.extend(cont.palettes.into_iter());
        self.palettes_found_in.extend(repeat(file_id).take(num_palettes));

        self.animations.extend(cont.animations.into_iter());
        self.animations_found_in.extend(repeat(file_id).take(num_animations));
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

    fn build_material_table(&mut self) {
        let mut missing = false;
        let mut clash = false;

        for (model_id, model) in self.models.iter().enumerate() {
            let file_id = self.models_found_in[model_id];
            for (material_id, material) in model.materials.iter().enumerate() {
                let desc =
                    if material.texture_name.is_none() {
                        ImageDesc::NoImage
                    } else {
                        let texture_name = material.texture_name.as_ref().unwrap();

                        let best_texture = self.find_best_texture(texture_name, file_id, &mut clash);
                        let best_palette = material.palette_name
                            .and_then(|ref name| self.find_best_palette(name, file_id, &mut clash));

                        match best_texture {
                            None => {
                                missing = true;
                                ImageDesc::Missing
                            }
                            Some(texture_id) => ImageDesc::Image {
                                texture_id, palette_id: best_palette,
                            }
                        }
                    };
                self.material_table.insert((model_id, material_id), desc);
            }
        }

        if missing {
            warn!("couldn't find a texture/palette for some materials. Some textures \
                may be missing in your model.");
            info!("to fix this, try providing more files (textures may be stored in a \
                separate .nsbtx file)");
        }
        if clash {
            warn!("there were multiple textures/palettes with the same name. The \
                one we picked might not be the right one. Some textures may be wrong \
                in your model.");
            info!("to fix this, try providing fewer files at once");
        }
    }

    // The next two functions look for the best texture/palette of a given name.
    // Ones in the same file as the model we're looking at are considered better
    // than ones that aren't. The `clash` variable is set to true if there are
    // multiple possible candidates that are as good as one another.

    fn find_best_texture(&self, name: &Name, file_id: FileId, clash: &mut bool)
    -> Option<TextureId> {
        let candidates = self.textures_by_name.get(name)?;

        if candidates.len() == 1 { return Some(candidates[0]); }

        // Try to find one from the same file first.
        let mut candidates_from_same_file = candidates.iter()
            .filter(|&&id| self.textures_found_in[id] == file_id);
        if let Some(&can) = candidates_from_same_file.next() {
            if candidates_from_same_file.next().is_some() {
                *clash = true;
                warn!("multiple textures named {:?} in the same file", name);
            }
            return Some(can);
        }

        // Just give the first one
        *clash = true;
        warn!("multiple textures named {:?}; using the first one", name);
        Some(candidates[0])
    }

    fn find_best_palette(&self, name: &Name, file_id: FileId, clash: &mut bool)
    -> Option<PaletteId> {
        let candidates = self.palettes_by_name.get(name)?;

        if candidates.len() == 1 { return Some(candidates[0]); }

        // Try to find one from the same file first.
        let mut candidates_from_same_file = candidates.iter()
            .filter(|&&id| self.palettes_found_in[id] == file_id);
        if let Some(&can) = candidates_from_same_file.next() {
            if candidates_from_same_file.next().is_some() {
                *clash = true;
                warn!("multiple palettes named {:?} in the same file", name);
            }
            return Some(can);
        }

        // Just give the first one
        *clash = true;
        warn!("multiple palettes named {:?}; using the first one", name);
        Some(candidates[0])
    }
}


fn read_file(path: &Path) -> Result<Vec<u8>> {
    use std::fs::File;
    use std::io::Read;
    let mut f = File::open(&path)?;
    let mut b: Vec<u8> = vec![];
    f.read_to_end(&mut b)?;
    Ok(b)
}
