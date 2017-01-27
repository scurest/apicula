//! Nitro model.
//!
//! A Nitro model is rendered by interpretting a list of "render commands".
//! The rest of the data in this file is useful only insofar as it is referenced
//! from these commands. Typical commands are "multiply current matrix by object
//! matrix and store to GPU", "bind material", and "render mesh".
//!
//! A mesh is just a list of GPU commands which draw some geometry. Rendering
//! a mesh on the DS is easy: you just transfer all the commands to the GPU.
//!
//! A material is the set of data needed to set up the texture, palette, and
//! lighting states on the GPU. The texture and palette are referenced by name
//! alone. You need to search a TEX file for a matching name to find the needed
//! data.
//!
//! An object is a matrix which the render commands can multiply by and store
//! to the GPU's matrix stack. They are basically joints (eg. for a character,
//! names "Arm", "Leg", etc. are typical). They are multipled to form a joint
//! hierarchy and the commands in a mesh reference those multiplied matrices
//! stored in the stack.
//!
//! Animation is performed by executing the same rendering commands, just using
//! different object matrices than the ones stored here. The ones stored here
//! give the model's rest pose. See the `jnt` module for animation info.
//!
//! ------
//!
//! How is a model divided up into multiple meshes? Each mesh uses exactly one
//! material and up to 32 matrix stack slots. Since 32 matrices are typically
//! enough, the main limiting constraint is the first: there will generally be
//! one mesh per material. Division into meshes can also be a form of reuse:
//! a model with two copies of a sphere could use one mesh rendered twice with
//! different matrix setups in between.

pub mod render_cmds;
mod read;
mod xform;

use cgmath::Matrix3;
use cgmath::Matrix4;
use nitro::name::Name;
use nitro::tex::TextureParameters;
use util::cur::Cur;

pub use self::read::read_mdl;
pub use self::xform::pivot_mat;

#[derive(Debug, Clone)]
pub struct Mdl<'a> {
    pub models: Vec<Model<'a>>,
}

#[derive(Debug, Clone)]
pub struct Model<'a> {
    pub name: Name,
    pub materials: Vec<Material>,
    pub meshes: Vec<Mesh<'a>>,
    pub objects: Vec<Object>,
    pub blend_matrices: Vec<BlendMatrixPair>,
    pub render_cmds_cur: Cur<'a>,
    pub up_scale: f64,
    pub down_scale: f64,
}

#[derive(Debug, Clone)]
pub struct Material {
    pub name: Name,
    pub texture_name: Option<Name>,
    pub palette_name: Option<Name>,
    pub params: TextureParameters,
    pub width: u16,
    pub height: u16,
    pub texture_mat: Matrix4<f64>,
}

#[derive(Debug, Clone)]
pub struct Mesh<'a> {
    pub name: Name,
    pub commands: &'a [u8],
}

#[derive(Debug, Clone)]
pub struct Object {
    pub name: Name,
    pub xform: Matrix4<f64>,
}

/// A pair of matrices used for the blending render command (opcode 0x09).
///
/// The first one is used in calculating vertex positions. The second is used
/// for normals (?) which we don't currently handle.
#[derive(Debug, Clone)]
pub struct BlendMatrixPair(pub Matrix4<f64>, pub Matrix3<f64>);
