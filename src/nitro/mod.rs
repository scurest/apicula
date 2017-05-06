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

pub mod container;
pub mod gpu_cmds;
pub mod jnt;
pub mod mdl;
pub mod name;
pub mod tex;
mod info_block;
mod rotation;
