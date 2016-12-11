mod model;
mod xform;

use cgmath::Matrix4;
use nitro::tex::TextureParameters;
use util::cur::Cur;
use util::name::Name;

pub use self::model::read_mdl;

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
    pub render_cmds_cur: Cur<'a>,
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
