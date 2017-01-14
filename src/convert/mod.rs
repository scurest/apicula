#[macro_use]
mod format;
mod collada;

use clap::ArgMatches;
use errors::Result;
use files::BufferHolder;
use files::FileHolder;
use nitro::mdl::Material;
use nitro::name;
use nitro::name::Name;
use nitro::tex;
use png;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use util::uniq::UniqueNamer;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct TexPalPair(Name, Option<Name>);

impl TexPalPair {
    pub fn from_material(mat: &Material) -> Option<TexPalPair> {
        mat.texture_name
            .map(|texture_name| TexPalPair(texture_name, mat.palette_name))
    }
}

impl ToString for TexPalPair {
    fn to_string(&self) -> String {
        format!("{}", self.0)
    }
}

pub fn main(matches: &ArgMatches) -> Result<()> {
    let input_files = matches
        .values_of_os("INPUT").unwrap();
    let buf_holder = BufferHolder::read_files(input_files)?;
    let file_holder = FileHolder::from_buffers(&buf_holder);

    let model = &file_holder.models[0];
    let tex = &file_holder.texs[0];

    let out_dir = PathBuf::from(matches.value_of("OUTPUT").unwrap());
    fs::create_dir(&out_dir)?;
    let dae_path = out_dir.join(&format!("{}.dae", name::IdFmt(&model.name)));
    let mut f = File::create(dae_path)?;

    let mut image_namer = UniqueNamer::new();

    let mut s = String::new();
    collada::write(&mut s, model, &file_holder.animations[..], &mut image_namer)?;

    f.write_all(s.as_bytes())?;

    for (pair, image_name) in image_namer.map().iter() {
        let texture_name = pair.0;
        let texinfo = tex.texinfo.iter()
            .find(|info| info.name == texture_name);
        let texinfo = match texinfo {
            Some(info) => info,
            None => {
                warn!("couldn't find a texture named: {}", texture_name);
                continue;
            }
        };

        let palette_name = pair.1;
        let palinfo = palette_name.and_then(|palname|
            tex.palinfo.iter()
                .find(|info| info.name == palname)
        );

        let res = tex::image::gen_image(tex, texinfo, palinfo);
        let rgba = match res {
            Ok(rgba) => rgba,
            Err(e) => {
                warn!("error generating image {}, error: {:#?}", image_name, e);
                continue;
            }
        };

        let path = out_dir.join(&format!("{}.png", image_name));
        png::write(&path, &rgba[..], texinfo.params.width(), texinfo.params.height())?;
    }

    Ok(())
}
