//! Nitro containers.
//!
//! A Nitro container holds... well, subcontainers, but inside of _those_ are
//! the model, texture, or animation files. A Nitro container is recognized by
//! an identifying stamp: eg. b"BMD0". The subcontainers also contain a stamp
//! eg. b"MDL0". An MDL0 contains models, a TEX0 contains textures and palettes,
//! and a JNT0 contains animations.
//!
//! The different types of containers are supposed to only contain certain kinds
//! of subcontainers. For example, a BMD0 (a "Nitro model") typically only
//! contains MDL0s and TEX0s (ie. models, textures and palettes) and a BCA0
//! usually only contains JNT0s (animations), but we don't do anything to
//! enforce this. We'll read any kind of file we can get our hands on!

use crate::errors::Result;
use crate::nitro::{Model, Texture, Palette, Animation, Pattern, MaterialAnimation};
use crate::nitro::info_block;
use crate::util::cur::Cur;

const STAMPS: [&[u8]; 5] = [b"BMD0", b"BTX0", b"BCA0", b"BTP0", b"BTA0"];

pub struct Container {
    pub stamp: &'static [u8],
    pub file_size: u32,
    pub models: Vec<Model>,
    pub textures: Vec<Texture>,
    pub palettes: Vec<Palette>,
    pub animations: Vec<Animation>,
    pub patterns: Vec<Pattern>,
    pub mat_anims: Vec<MaterialAnimation>,
}

pub fn read_container(cur: Cur) -> Result<Container> {
    fields!(cur, container {
        stamp: [u8; 4],
        bom: u16,
        version: u16,
        file_size: u32,
        header_size: u16,
        num_sections: u16,
        section_offs: [u32; num_sections],
    });

    let stamp =
        match STAMPS.iter().find(|&s| s == &stamp) {
            Some(x) => x,
            None => bail!("unrecognized Nitro container: expected \
                the first four bytes to be one of: BMD0, BTX0, BCA0, BTP0, BTA0"),
        };

    check!(bom == 0xfeff)?;
    check!(header_size == 16)?;
    check!(file_size > 16)?;

    let mut cont = Container {
        stamp, file_size, models: vec![], textures: vec![],
        palettes: vec![], animations: vec![], patterns: vec![],
        mat_anims: vec![],
    };

    for section_off in section_offs {
        let section_cur = cur + section_off;
        if let Err(e) = read_section(&mut cont, section_cur) {
            debug!("skipping Nitro section: {}", e);
        }
    }

    Ok(cont)
}

fn read_section(cont: &mut Container, cur: Cur) -> Result<()> {
    let stamp = cur.clone().next_n_u8s(4)?;
    match stamp {
        b"MDL0" => add_mdl(cont, cur),
        b"TEX0" => add_tex(cont, cur),
        b"JNT0" => add_jnt(cont, cur),
        b"PAT0" => add_pat(cont, cur),
        b"SRT0" => add_srt(cont, cur),
        _ => bail!("unrecognized Nitro format: expected the first four \
            bytes to be one of: MDL0, TEX0, JNT0, PAT0, SRT0"),
    }
}

// An MDL is a container for models.
fn add_mdl(cont: &mut Container, cur: Cur) -> Result<()> {
    use crate::nitro::model::read_model;

    fields!(cur, MDL0 {
        stamp: [u8; 4],
        section_size: u32,
        end: Cur,
    });
    check!(stamp == b"MDL0")?;

    for (off, name) in info_block::read::<u32>(end)? {
        match read_model(cur + off, name) {
            Ok(model) => cont.models.push(model),
            Err(e) => {
                error!("error on model {}: {}", name, e);
            }
        }
    }
    Ok(())
}

// This work is already done for us in read_tex; see that module for why.
fn add_tex(cont: &mut Container, cur: Cur) -> Result<()> {
    use crate::nitro::tex::read_tex;

    let (textures, palettes) = read_tex(cur)?;
    cont.textures.extend(textures.into_iter());
    cont.palettes.extend(palettes.into_iter());
    Ok(())
}


// A JNT is a container for animations.
fn add_jnt(cont: &mut Container, cur: Cur) -> Result<()> {
    use crate::nitro::animation::read_animation;

    fields!(cur, JNT0 {
        stamp: [u8; 4],
        section_size: u32,
        end: Cur,
    });
    check!(stamp == b"JNT0")?;

    for (off, name) in info_block::read::<u32>(end)? {
        match read_animation(cur + off, name) {
            Ok(animation) => cont.animations.push(animation),
            Err(e) => {
                error!("error on animation {}: {}", name, e);
            }
        }
    }
    Ok(())
}

// A PAT is a container for pattern animations.
fn add_pat(cont: &mut Container, cur: Cur) -> Result<()> {
    use crate::nitro::pattern::read_pattern;

    fields!(cur, PAT0 {
        stamp: [u8; 4],
        section_size: u32,
        end: Cur,
    });
    check!(stamp == b"PAT0")?;

    for (off, name) in info_block::read::<u32>(end)? {
        match read_pattern(cur + off, name) {
            Ok(pattern) => cont.patterns.push(pattern),
            Err(e) => {
                error!("error on pattern {}: {}", name, e);
            }
        }
    }
    Ok(())
}

// An SRT is a container for material animations.
fn add_srt(cont: &mut Container, cur: Cur) -> Result<()> {
    use crate::nitro::material_animation::read_mat_anim;

    fields!(cur, SRT0 {
        stamp: [u8; 4],
        section_size: u32,
        end: Cur,
    });
    check!(stamp == b"SRT0")?;

    for (off, name) in info_block::read::<u32>(end)? {
        match read_mat_anim(cur + off, name) {
            Ok(mat_anim) => cont.mat_anims.push(mat_anim),
            Err(e) => {
                error!("error on material animation {}: {}", name, e);
            }
        }
    }
    Ok(())
}
