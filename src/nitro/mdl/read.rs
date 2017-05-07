use cgmath::Matrix3;
use cgmath::Matrix4;
use cgmath::One;
use cgmath::vec3;
use errors::Result;
use nitro::info_block;
use nitro::mdl::Material;
use nitro::mdl::Mdl;
use nitro::mdl::Mesh;
use nitro::mdl::Model;
use nitro::mdl::Object;
use nitro::name::Name;
use nitro::rotation::pivot_mat;
use nitro::tex::TextureParameters;
use util::bits::BitField;
use util::cur::Cur;

pub fn read_mdl(cur: Cur) -> Result<Mdl> {
    fields!(cur, MDL0 {
        stamp: [u8; 4],
        section_size: u32,
        end: Cur,
    });
    check!(stamp == b"MDL0")?;

    let models = info_block::read::<u32>(end)?
        .map(|(off, name)| read_model((cur + off as usize)?, name))
        .collect::<Result<_>>()?;

    Ok(Mdl { models })
}

fn read_model(cur: Cur, name: Name) -> Result<Model> {
    trace!("model: {}", name);
    fields!(cur, model {
        section_size: u32,
        render_cmds_off: u32,
        materials_off: u32,
        mesh_off: u32,
        inv_bind_matrices_off: u32,
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
        end: Cur,
    });

    let render_cmds_cur = (cur + render_cmds_off as usize)?;
    let objects = read_objects(end)?;
    let materials = read_materials((cur + materials_off as usize)?)?;
    let meshes = read_meshes((cur + mesh_off as usize)?)?;
    let inv_bind_matrices_cur = (cur + inv_bind_matrices_off as usize)?;

    Ok(Model {
        name,
        materials,
        meshes,
        objects,
        inv_bind_matrices_cur,
        render_cmds_cur,
        up_scale,
        down_scale,
    })
}

fn read_meshes(cur: Cur) -> Result<Vec<Mesh>> {
    info_block::read::<u32>(cur)?
        .map(|(off, name)| read_mesh((cur + off as usize)?, name))
        .collect()
}

fn read_mesh(cur: Cur, name: Name) -> Result<Mesh> {
    trace!("mesh: {}", name);
    fields!(cur, mesh {
        dummy: u16,
        section_size: u16,
        unknown: u32,
        commands_off: u32,
        commands_len: u32,
    });
    check!(section_size == 16)?;
    check!(commands_len % 4 == 0)?;

    let commands = (cur + commands_off as usize)?
        .next_n_u8s(commands_len as usize)?;

    Ok(Mesh { name, commands })
}

fn read_materials(cur: Cur) -> Result<Vec<Material>> {
    fields!(cur, materials {
        texture_pairing_off: u16,
        palette_pairing_off: u16,
        end: Cur,
    });

    let mut materials = info_block::read::<u32>(end)?
        .map(|(off, name)| read_material((cur + off as usize)?, name))
        .collect::<Result<Vec<_>>>()?;

    // Pair each texture with materials.
    let tex_cur = (cur + texture_pairing_off as usize)?;
    for ((off, num, _), name) in info_block::read::<(u16, u8, u8)>(tex_cur)? {
        trace!("texture pairing: {}", name);
        fields!((cur + off as usize)?, texture_pairings {
            material_ids: [u8; num],
        });
        for &mat_id in material_ids {
            materials[mat_id as usize].texture_name = Some(name);
        }
    }

    // Pair each palette with materials.
    let pal_cur = (cur + palette_pairing_off as usize)?;
    for ((off, num, _), name) in info_block::read::<(u16, u8, u8)>(pal_cur)? {
        trace!("palette pairing: {}", name);
        fields!((cur + off as usize)?, palette_pairings {
            material_ids: [u8; num],
        });
        for &mat_id in material_ids {
            materials[mat_id as usize].palette_name = Some(name);
        }
    }

    Ok(materials)
}

fn read_material(cur: Cur, name: Name) -> Result<Material> {
    trace!("material: {:?}", name);
    fields!(cur, material {
        dummy: u16,
        section_size: u16,
        dif_amb: u32,
        spe_emi: u32,
        polygon_attr: u32,
        unknown2: u32,
        params: u32,
        unknown3: [u32; 2],
        width: u16,
        height: u16,
        end: Cur,
    });

    let params = TextureParameters(params);

    let texture_mat = match params.texcoord_transform_mode() {
        0 => Matrix4::from_scale(1.0),
        1 => {
            // This is probably wrong. It might also be 8 fix16s.
            // But it handles the common case with a3=a4=2 for
            // mirrored textures.
            fields!(end, texcoord_matrix {
                a1: (fix32(1,19,12)), // always 1?
                a2: (fix32(1,19,12)), // always 1?
                a3: (fix32(1,19,12)),
                a4: (fix32(1,19,12)),
            });
            Matrix4::from_nonuniform_scale(a3, a4, 1.0)
        }
        2 | 3 => unimplemented!(),
        _ => unreachable!(),
    };

    Ok(Material {
        name,
        texture_name: None,
        palette_name: None,
        params,
        width,
        height,
        texture_mat,
    })
}

fn read_objects(cur: Cur) -> Result<Vec<Object>> {
    info_block::read::<u32>(cur)?
        .map(|(off, name)| read_object((cur + off as usize)?, name))
        .collect()
}

fn read_object(cur: Cur, name: Name) -> Result<Object> {
    trace!("object: {}", name);
    fields!(cur, object_transform {
        flags: u16,
        m0: (fix16(1,3,12)),
        end: Cur,
    });

    let t = flags.bits(0,1);
    let r = flags.bits(1,2);
    let s = flags.bits(2,3);
    let p = flags.bits(3,4);

    let mut cur = end;
    let mut xform = Matrix4::one();

    // Translation
    if t == 0 {
        fields!(cur, translation {
            x: (fix32(1,19,12)),
            y: (fix32(1,19,12)),
            z: (fix32(1,19,12)),
            end: Cur,
        });
        cur = end;

        let translation = Matrix4::from_translation(vec3(x, y, z));
        xform = translation;
    }

    // 3x3 Matrix (typically a rotation)
    if p == 1 {
        fields!(cur, pivot_rot {
            a: (fix16(1,3,12)),
            b: (fix16(1,3,12)),
            end: Cur,
        });
        cur = end;

        let select = flags.bits(4,8);
        let neg = flags.bits(8,12);
        let rotation = pivot_mat(select, neg, a, b)?;
        xform = xform * rotation;
    }
    if p == 0 && r == 0 {
        fields!(cur, rot {
            m1: (fix16(1,3,12)),
            m2: (fix16(1,3,12)),
            m3: (fix16(1,3,12)),
            m4: (fix16(1,3,12)),
            m5: (fix16(1,3,12)),
            m6: (fix16(1,3,12)),
            m7: (fix16(1,3,12)),
            m8: (fix16(1,3,12)),
            end: Cur,
        });
        cur = end;
        let matrix: Matrix4<_> = Matrix3::new(
            m0,  m1,  m2,
            m3,  m4,  m5,
            m6,  m7,  m8,
        ).into();
        xform = xform * matrix;
    }

    // Scale
    if s == 0 {
        fields!(cur, scale {
            sx: (fix32(1,19,12)),
            sy: (fix32(1,19,12)),
            sz: (fix32(1,19,12)),
        });
        let scale = Matrix4::from_nonuniform_scale(sx, sy, sz);
        xform = xform * scale;
    }

    Ok(Object { name, xform })
}
