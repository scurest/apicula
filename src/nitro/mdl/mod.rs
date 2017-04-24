//! Nitro model.
//!
//! A Nitro model is rendered by interpreting a list of "render commands".
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
//! names "Arm", "Leg", etc. are typical). They are multiplied to form a joint
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

use cgmath::Matrix3;
use cgmath::Matrix4;
use nitro::name::Name;
use nitro::tex::TextureParameters;
use util::cur::Cur;
use util::fixed::fix32;
use util::view::Viewable;

pub use self::read::read_mdl;

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
    pub inv_bind_matrices_cur: Cur<'a>,
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

/// The first matrix is an inverse bind matrix for some local-to-world
/// transform used by render command 0x09; see the comment there for
/// details. I don't know what the second is for (normals??).
#[derive(Debug, Clone)]
pub struct InvBindMatrixPair(pub Matrix4<f64>, pub Matrix3<f64>);

impl Viewable for InvBindMatrixPair {
    fn size() -> usize {
        // One 4 x 3 = 12 matrix and one 3 x 3 = 9 matrix
        // Each entry is 4 bytes (for a 1,19,12-format fixed point number)
        (12 + 9) * 4
    }

    fn view(buf: &[u8]) -> InvBindMatrixPair {
        let mut cur = Cur::new(buf);
        let entries = cur.next_n::<u32>(12 + 9).unwrap();
        let get = |i| fix32(entries.get(i), 1, 19, 12);

        let m0 = Matrix4::new(
            get(0), get(1), get(2), 0.0,
            get(3), get(4), get(5), 0.0,
            get(6), get(7), get(8), 0.0,
            get(9), get(10), get(11), 1.0,
        );
        let m1 = Matrix3::new(
            get(12), get(13), get(14),
            get(15), get(16), get(17),
            get(18), get(19), get(20),
        );

        InvBindMatrixPair(m0, m1)
    }
}
