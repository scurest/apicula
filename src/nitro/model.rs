use cgmath::{Matrix3, Matrix4, One, vec3, Vector3};
use errors::Result;
use nitro::info_block;
use nitro::Name;
use nds::TextureParams;
use nitro::render_cmds::Op;
use util::bits::BitField;
use util::cur::Cur;
use util::fixed::{fix16, fix32};

/// NSBMD model. Consists of data like meshes and materials, and a list of
/// rendering commands. Drawn by executing the rendering commands.
pub struct Model {
    pub name: Name,
    pub materials: Vec<Material>,
    pub meshes: Vec<Mesh>,
    pub objects: Vec<Object>,
    pub inv_binds: Vec<Matrix4<f64>>,
    pub render_ops: Vec<Op>,
    pub up_scale: f64,
    pub down_scale: f64,
}

pub fn read_model(cur: Cur, name: Name) -> Result<Model> {
    debug!("model: {:?}", name);

    fields!(cur, model {
        section_size: u32,
        render_cmds_off: u32,
        materials_off: u32,
        mesh_off: u32,
        inv_binds_off: u32,
        unknown1: [u8; 3],
        num_objects: u8,
        num_materials: u8,
        num_meshes: u8,
        unknown2: [u8; 2],
        up_scale: (fix32(1,19,12)),
        down_scale: (fix32(1,19,12)),
        num_verts: u16,
        num_surfs: u16,
        num_tris: u16,
        num_quads: u16,
        bounding_box_x_min: (fix16(1,3,12)),
        bounding_box_y_min: (fix16(1,3,12)),
        bounding_box_z_min: (fix16(1,3,12)),
        bounding_box_x_max: (fix16(1,3,12)),
        bounding_box_y_max: (fix16(1,3,12)),
        bounding_box_z_max: (fix16(1,3,12)),
        unknown3: [u8; 8],
        objects_cur: Cur,
    });

    use super::render_cmds::parse_render_cmds;
    let render_ops = parse_render_cmds(cur + render_cmds_off)?;

    let meshes = read_meshes(cur + mesh_off)?;
    let materials = read_materials(cur + materials_off)?;
    let objects = read_objects(objects_cur)?;
    let inv_binds = read_inv_binds(cur + inv_binds_off, num_objects as usize);

    let model = Model {
        name, materials, meshes, objects, inv_binds,
        render_ops, up_scale, down_scale,
    };

    validate_render_ops(&model)?;

    Ok(model)
}

/// Validate that all the indices in the render ops are in-bounds.
fn validate_render_ops(model: &Model) -> Result<()> {
    for op in &model.render_ops {
        let good = match *op {
            Op::MulObject { object_idx } => (object_idx as usize) < model.objects.len(),
            Op::BindMaterial { material_idx } => (material_idx as usize) < model.materials.len(),
            Op::Draw { mesh_idx } => (mesh_idx as usize) < model.meshes.len(),
            Op::Skin { ref terms } => {
                terms.iter().all(|term| {
                    (term.inv_bind_idx as usize) < model.inv_binds.len()
                })
            }
            _ => true,
        };
        if !good {
            bail!("model had out-of-bounds index in render commands");
        }
    }
    Ok(())
}

/// A mesh is a "piece" of a model, containing the actual vertex/polygon data.
/// It is really just a blob of NDS GPU commands, drawn by just submitting the
/// blob to the GPU.
pub struct Mesh {
    pub name: Name,
    pub gpu_commands: Vec<u8>,
}

fn read_meshes(cur: Cur) -> Result<Vec<Mesh>> {
    info_block::read::<u32>(cur)?
        .map(|(off, name)| read_mesh(cur + off, name))
        .collect()
}

fn read_mesh(cur: Cur, name: Name) -> Result<Mesh> {
    debug!("mesh: {:?}", name);

    fields!(cur, mesh {
        dummy: u16,
        section_size: u16,
        unknown: u32,
        cmds_off: u32,
        cmds_len: u32,
    });

    check!(section_size == 16)?;
    check!(cmds_len % 4 == 0)?;

    let gpu_commands = (cur + cmds_off)
        .next_n_u8s(cmds_len as usize)?
        .to_vec();

    Ok(Mesh { name, gpu_commands })
}

/// Material contains drawing state, eg. texture name, whether to cull
/// backfacing polys, etc.
pub struct Material {
    pub name: Name,
    pub texture_name: Option<Name>,
    pub palette_name: Option<Name>,
    pub params: TextureParams,
    pub width: u16,
    pub height: u16,
    pub cull_backface: bool,
    pub cull_frontface: bool,
    pub texture_mat: Matrix4<f64>,
}

fn read_materials(cur: Cur) -> Result<Vec<Material>> {
    fields!(cur, materials {
        texture_pairing_off: u16,
        palette_pairing_off: u16,
        end: Cur,
    });

    let mut materials = info_block::read::<u32>(end)?
        .map(|(off, name)| read_material(cur + off, name))
        .collect::<Result<Vec<_>>>()?;

    // Pair each texture with materials.
    let tex_cur = cur + texture_pairing_off;
    for ((off, num, _), name) in info_block::read::<(u16, u8, u8)>(tex_cur)? {
        trace!("texture pairing: {}", name);
        fields!(cur + off, texture_pairings {
            material_ids: [u8; num],
        });
        for &mat_id in material_ids {
            materials[mat_id as usize].texture_name = Some(name);
        }
    }

    // Pair each palette with materials.
    let pal_cur = cur + palette_pairing_off;
    for ((off, num, _), name) in info_block::read::<(u16, u8, u8)>(pal_cur)? {
        trace!("palette pairing: {}", name);
        fields!(cur + off, palette_pairings {
            material_ids: [u8; num],
        });
        for &mat_id in material_ids {
            materials[mat_id as usize].palette_name = Some(name);
        }
    }

    // NOTE: This apparently bizarre way of associating texture/palette names
    // with materials suggests that there's something going on here we don't
    // know about. Possibly related to whatever mechanism the run-time actually
    // uses to resolve the names to the actual textures/palettes?

    Ok(materials)
}

fn read_material(cur: Cur, name: Name) -> Result<Material> {
    debug!("material: {:?}", name);

    fields!(cur, material {
        dummy: u16,
        section_size: u16,
        dif_amb: u32,
        spe_emi: u32,
        polygon_attr: u32,
        unknown2: u32,
        params: u32,
        unknown3: u32,
        unknown4: u32, // flag for texture matrix?
        width: u16,
        height: u16,
        unknown5: (fix32(1,19,12)), // always 1?
        unknown6: (fix32(1,19,12)), // always 1?
        end: Cur,
    });

    let params = TextureParams(params);

    let cull_backface = polygon_attr.bits(6,7) == 0;
    let cull_frontface = polygon_attr.bits(7,8) == 0;

    // For now, use the section size to determine whether there
    // is texture matrix data.
    let texture_mat = match section_size {
        60 => {
            fields!(end, texcoord_matrix {
                a: (fix32(1,19,12)),
                b: (fix32(1,19,12)),
            });
            Matrix4::from_nonuniform_scale(a, b, 1.0)
        }
        _ => Matrix4::from_scale(1.0),
    };

    Ok(Material {
        name,
        texture_name: None,
        palette_name: None,
        params,
        width,
        height,
        cull_backface,
        cull_frontface,
        texture_mat,
    })
}

/// An object, basically just a matrix, typically corresponding to a single bone
/// in a skeleton. The value stored here is the rest pose. When the model is
/// animated, its the object matrices that change.
pub struct Object {
    pub name: Name,
    pub trans: Option<Vector3<f64>>,
    pub rot: Option<Matrix3<f64>>,
    pub scale: Option<Vector3<f64>>,

    /// Matrix for the above TRS transform.
    pub matrix: Matrix4<f64>,
}

fn read_objects(cur: Cur) -> Result<Vec<Object>> {
    info_block::read::<u32>(cur)?
        .map(|(off, name)| read_object(cur + off, name))
        .collect()
}

fn read_object(mut cur: Cur, name: Name) -> Result<Object> {
    let fx16 = |x| fix16(x, 1, 3, 12);
    let fx32 = |x| fix32(x, 1, 19, 12);

    trace!("object: {}", name);

    let flags = cur.next::<u16>()?;
    let t = flags.bits(0,1);
    let r = flags.bits(1,2);
    let s = flags.bits(2,3);
    let p = flags.bits(3,4);
    trace!("t={}, r={}, s={}, p={}", t, r, s, p);

    // Why in God's name is this here instead of with the
    // other rotation stuff?
    let m0 = cur.next::<u16>()?;

    let mut trans = None;
    let mut rot = None;
    let mut scale = None;

    // Translation
    if t == 0 {
        let (x, y, z) = cur.next::<(u32, u32, u32)>()?;
        trans = Some(vec3(fx32(x), fx32(y), fx32(z)));
    }
    trace!("trans: {:?}", trans);

    // 3x3 Matrix (typically rotation)
    if p == 1 {
        let a = fx16(cur.next::<u16>()?);
        let b = fx16(cur.next::<u16>()?);
        let select = flags.bits(4,8);
        let neg = flags.bits(8,12);
        use nitro::rotation::pivot_mat;
        rot = Some(pivot_mat(select, neg, a, b));
    } else if r == 0 {
        let m = cur.next_n::<u16>(8)?;
        rot = Some(Matrix3::new(
            fx16(m0), fx16(m.nth(0)), fx16(m.nth(1)),
            fx16(m.nth(2)), fx16(m.nth(3)), fx16(m.nth(4)),
            fx16(m.nth(5)), fx16(m.nth(6)), fx16(m.nth(7)),
        ));
    }
    trace!("rot: {:?}", rot);

    // Scale
    if s == 0 {
        let (x, y, z) = cur.next::<(u32, u32, u32)>()?;
        scale = Some(vec3(fx32(x), fx32(y), fx32(z)));
    }
    trace!("scale: {:?}", scale);

    // Compute TRS matrix
    let mut matrix = Matrix4::one();
    if let Some(s) = scale {
        matrix = Matrix4::from_nonuniform_scale(s.x, s.y, s.z);
    }
    if let Some(r) = rot {
        matrix = Matrix4::from(r) * matrix;
    }
    if let Some(t) = trans {
        matrix = Matrix4::from_translation(t) * matrix;
    }

    Ok(Object { name, trans, rot, scale, matrix })
}


/// Read inverse bind matrices. Each seems to be the inverse bind matrix for the
/// corresponding object (=bone) in the skeleton. A model only needs them if it
/// uses skinning commands. Some models don't have them and other models don't
/// need them but have them anyway.
///
/// We just read as many (up to num_objects) as we can get. (Thus why we don't
/// need to return a Result.)
fn read_inv_binds(mut cur: Cur, num_objects: usize) -> Vec<Matrix4<f64>> {
    // Each element in the inv bind array consists of
    // * one 4 x 3 matrix, the inverse of some local-to-world object
    //  transform; this is the one we care about
    // * one 3 x 3 matrix, possibly for normals(?) that we ignore
    // Each matrix entry is a 4-byte fixed point number.
    let elem_size = (4*3 + 3*3) * 4;

    let mut inv_binds = Vec::<Matrix4<f64>>::with_capacity(num_objects);
    for _ in 0..num_objects {
        if cur.bytes_remaining() < elem_size { break; }

        let entries = cur.next_n::<u32>(4*3).unwrap();
        let m = |i| fix32(entries.nth(i), 1, 19, 12);
        inv_binds.push(Matrix4::new(
            m(0), m(1), m(2), 0.0,
            m(3), m(4), m(5), 0.0,
            m(6), m(7), m(8), 0.0,
            m(9), m(10), m(11), 1.0,
        ));
        cur.jump_forward(3*3*4);
    }
    inv_binds
}
