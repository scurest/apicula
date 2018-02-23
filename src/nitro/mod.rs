//! Load Nitro files.
//!
//! Nitro is the SDK used for many Nintendo DS games. These modules parse
//! the binary format for Nitro files into domain objects and provide other
//! tools specific to these formats.
//!
//! The file types are
//!
//! * MDL - a 3D model file
//! * TEX - textures and palettes for models
//! * JNT - skinned vertex animations for models
//!
//! A container file (eg. NSBMD) holds one or more of these.

pub mod model;
pub mod tex;
pub mod animation;
pub mod container;
pub mod name;
pub mod render_cmds;
pub mod decode_image;
mod info_block;
mod rotation;

pub use self::{
    name::Name,
    container::Container,
    model::Model,
    tex::Texture,
    tex::Palette,
    tex::TextureParameters,
    animation::Animation,
};
