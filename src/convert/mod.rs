#[macro_use]
mod format;
mod collada;
mod image_names;

use clap::ArgMatches;
use errors::Result;
use nitro::tex;
use png;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use util::name;
use files::BufferHolder;
use files::FileHolder;

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

    let image_names = image_names::build_image_names(model);

    let mut s = String::new();
    collada::write(&mut s, model, &file_holder.animations[..], &image_names)?;

    f.write_all(s.as_bytes())?;

    for (texpal, image_name) in image_names.into_iter() {
        let texinfo = tex.texinfo.iter()
            .find(|info| info.name == texpal.texture_name);
        let texinfo = match texinfo {
            Some(info) => info,
            None => {
                warn!("couldn't find a texture named: {}", texpal.texture_name);
                continue;
            }
        };
        let palinfo = texpal.palette_name.and_then(|palname|
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
