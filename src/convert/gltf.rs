use nitro::Model;
use db::{Database, ModelId};
use connection::Connection;
use primitives::{Primitives, PolyType};
use skeleton::Skeleton;
use super::image_namer::ImageNamer;
use cgmath::Matrix4;
use json::JsonValue;
use super::glb::{Glb, Buffer, ByteVec, VecExt};
use super::object_trs::ObjectTRSes;

static FRAME_LENGTH: f64 = 1.0 / 60.0; // 60 fps

struct Ctx<'a> {
    model_id: ModelId,
    model: &'a Model,
    db: &'a Database,
    conn: &'a Connection,
    image_namer: &'a ImageNamer,
    rest_trses: ObjectTRSes,
    objects: &'a [Matrix4<f64>],
    prims: &'a Primitives,
    skel: &'a Skeleton,
}

pub fn to_glb(
    db: &Database,
    conn: &Connection,
    image_namer: &ImageNamer,
    model_id: ModelId,
) -> Glb {
    let model = &db.models[model_id];

    let rest_trses = ObjectTRSes::for_model_at_rest(model);
    let objects = &rest_trses.objects.iter()
        .map(|trs| Matrix4::from(trs))
        .collect::<Vec<_>>();
    let prims = &Primitives::build(model, PolyType::Tris, objects);
    let skel = &Skeleton::build(model, objects);

    let ctx = Ctx { model_id, model, db, conn, image_namer, rest_trses, objects, prims, skel };

    let mut glb = Glb::new();

    mesh(&ctx, &mut glb);
    nodes(&ctx, &mut glb);
    materials(&ctx, &mut glb);

    glb
}

static UNSIGNED_BYTE: u32 = 5121;
static UNSIGNED_SHORT: u32 = 5123;
static FLOAT: u32 = 5126;

fn mesh(ctx: &Ctx, glb: &mut Glb) {
    let verts = &ctx.prims.vertices;

    // Positions
    // glTF wants the min/max, so compute that first
    let mut min = verts[0].position.clone();
    let mut max = verts[0].position.clone();
    for v in verts {
        for i in 0..3 {
            min[i] = min[i].min(v.position[i]);
            max[i] = max[i].max(v.position[i]);
        }
    }
    let pos_accessor = {
        let buf = glb.buffers.add(Buffer {
            alignment: 4,
            bytes: Vec::with_capacity(3 * verts.len() * 4),
        });
        let dat = &mut glb.buffers[buf].bytes;
        for v in verts {
            dat.push_f32(v.position[0]);
            dat.push_f32(v.position[1]);
            dat.push_f32(v.position[2]);
        }
        let buf_view = glb.gltf["bufferViews"].add(object!(
            "buffer" => buf,
            "byteLength" => dat.len(),
        ));
        glb.gltf["accessors"].add(object!(
            "bufferView" => buf_view,
            "type" => "VEC3",
            "componentType" => FLOAT,
            "count" => verts.len(),
            "min" => min.to_vec(),
            "max" => max.to_vec(),
        ))
    };

    // Texcoord
    let has_texcoords = ctx.prims.draw_calls.iter().any(|call| call.used_texcoords);
    let tex_accessor = if has_texcoords {
        let buf = glb.buffers.add(Buffer {
            alignment: 4,
            bytes: Vec::with_capacity(2 * verts.len() * 4),
        });
        let dat = &mut glb.buffers[buf].bytes;
        for v in verts {
            dat.push_f32(v.texcoord[0]);
            dat.push_f32(v.texcoord[1]);
        }
        let buf_view = glb.gltf["bufferViews"].add(object!(
            "buffer" => buf,
            "byteLength" => dat.len(),
        ));
        Some(glb.gltf["accessors"].add(object!(
            "bufferView" => buf_view,
            "type" => "VEC2",
            "componentType" => FLOAT,
            "count" => verts.len(),
        )))
    } else {
        None
    };

    // Color
    let has_colors = ctx.prims.draw_calls.iter().any(|call| call.used_vertex_color);
    let color_accessor = if has_colors {
        let buf = glb.buffers.add(Buffer {
            alignment: 1,
            bytes: Vec::with_capacity(3 * verts.len() * 1),
        });
        let dat = &mut glb.buffers[buf].bytes;
        for v in verts {
            dat.push_normalized_u8(v.color[0]);
            dat.push_normalized_u8(v.color[1]);
            dat.push_normalized_u8(v.color[2]);
        }
        let buf_view = glb.gltf["bufferViews"].add(object!(
            "buffer" => buf,
            "byteLength" => dat.len(),
        ));
        Some(glb.gltf["accessors"].add(object!(
            "bufferView" => buf_view,
            "type" => "VEC3",
            "componentType" => UNSIGNED_BYTE,
            "normalized" => true,
            "count" => verts.len(),
        )))
    } else {
        None
    };

    // Normals
    let has_normals = ctx.prims.draw_calls.iter().any(|call| call.used_normals);
    let normal_accessor = if has_normals {
        let buf = glb.buffers.add(Buffer {
            alignment: 4,
            bytes: Vec::with_capacity(3 * verts.len() * 4),
        });
        let dat = &mut glb.buffers[buf].bytes;
        for v in verts {
            dat.push_f32(v.normal[0]);
            dat.push_f32(v.normal[1]);
            dat.push_f32(v.normal[2]);
        }
        let buf_view = glb.gltf["bufferViews"].add(object!(
            "buffer" => buf,
            "byteLength" => dat.len(),
        ));
        Some(glb.gltf["accessors"].add(object!(
            "bufferView" => buf_view,
            "type" => "VEC3",
            "componentType" => FLOAT,
            "count" => verts.len(),
        )))
    } else {
        None
    };

    // Now joints/weights

    // glTF gives joint/weight influences in sets of 4 (JOINT_0 is a VEC4
    // accessor with the first four joints, JOINTS_1 has the next four, etc).
    // Find out how many sets we need.
    let num_sets = (ctx.skel.max_num_influences + 3) / 4;

    // Make sure joints fit in a byte
    assert!(ctx.skel.tree.node_count() <= 255);

    // Joints
    let joints_accessors = {
        let buf = glb.buffers.add(Buffer {
            alignment: 1,
            bytes: Vec::with_capacity(4 * num_sets * verts.len() * 1),
        });
        let dat_len = {
            let dat = &mut glb.buffers[buf].bytes;
            for sv in &ctx.skel.vertices {
                let mut i = 0;
                while i != 4 * num_sets {
                    if i < sv.influences.len() {
                        dat.push(sv.influences[i].joint.index() as u8);
                    } else {
                        dat.push(0);
                    }
                    i += 1;
                }
            }
            dat.len()
        };
        (0..num_sets).map(|set_num| {
            let buf_view = glb.gltf["bufferViews"].add(object!(
                "buffer" => buf,
                "byteOffset" => 4 * set_num,
                "byteStride" => 4 * num_sets,
                "byteLength" => dat_len - 4 * set_num,
            ));
            glb.gltf["accessors"].add(object!(
                "bufferView" => buf_view,
                "type" => "VEC4",
                "componentType" => UNSIGNED_BYTE,
                "count" => verts.len(),
            ))
        }).collect::<Vec<_>>()
    };

    // Weights
    let weights_accessors = {
        let buf = glb.buffers.add(Buffer {
            alignment: 1,
            bytes: Vec::with_capacity(4 * num_sets * verts.len() * 1),
        });
        let dat_len = {
            let dat = &mut glb.buffers[buf].bytes;
            for sv in &ctx.skel.vertices {
                let mut i = 0;
                while i != 4 * num_sets {
                    if i < sv.influences.len() {
                        dat.push_normalized_u8(sv.influences[i].weight);
                    } else {
                        dat.push_normalized_u8(0.0);
                    }
                    i += 1;
                }
            }
            dat.len()
        };
        (0..num_sets).map(|set_num| {
            let buf_view = glb.gltf["bufferViews"].add(object!(
                "buffer" => buf,
                "byteOffset" => 4 * set_num,
                "byteStride" => 4 * num_sets,
                "byteLength" => dat_len - 4 * set_num,
            ));
            glb.gltf["accessors"].add(object!(
                "bufferView" => buf_view,
                "type" => "VEC4",
                "componentType" => UNSIGNED_BYTE,
                "normalized" => true,
                "count" => verts.len(),
            ))
        }).collect::<Vec<_>>()
    };

    // Put the indices into a buffer view
    let index_buf_view = {
        let buf = glb.buffers.add(Buffer {
            alignment: 2,
            bytes: Vec::with_capacity(verts.len() * 2),
        });
        let dat = &mut glb.buffers[buf].bytes;
        for &ind in &ctx.prims.indices {
            dat.push_u16(ind);
        }
        glb.gltf["bufferViews"].add(object!(
            "buffer" => buf,
            "byteLength" => dat.len(),
        ))
    };

    // One glTF primitive per draw call
    let primitives = ctx.prims.draw_calls.iter().map(|call| {
        let indices_accessor = glb.gltf["accessors"].add(object!(
            "bufferView" => index_buf_view,
            "type" => "SCALAR",
            "byteOffset" => 2 * call.index_range.start,
            "componentType" => UNSIGNED_SHORT,
            "count" => call.index_range.len(),
        ));
        let mut primitive = object!(
            "attributes" => object!(
                "POSITION" => pos_accessor,
            ),
            "material" => call.mat_id,
            "indices" => indices_accessor,
        );
        if let Some(tex_accessor) = tex_accessor {
            primitive["attributes"]["TEXCOORD_0"] = tex_accessor.into();
        }
        if let Some(color_accessor) = color_accessor {
            primitive["attributes"]["COLOR_0"] = color_accessor.into();
        }
        if let Some(normal_accessor) = normal_accessor {
            primitive["attributes"]["NORMAL"] = normal_accessor.into();
        }
        for (set_num, &joints_accessor) in joints_accessors.iter().enumerate() {
            primitive["attributes"][format!("JOINTS_{}", set_num)] = joints_accessor.into();
        }
        for (set_num, &weights_accessor) in weights_accessors.iter().enumerate() {
            primitive["attributes"][format!("WEIGHTS_{}", set_num)] = weights_accessor.into();
        }
        primitive
    }).collect::<Vec<JsonValue>>();

    glb.gltf["meshes"] = array!(
        object!(
            "primitives" => primitives,
            "name" => ctx.model.name.to_string(),
        )
    );
}

fn nodes(ctx: &Ctx, glb: &mut Glb) {
    // NOTE: the NodeIndices for skel.tree are the same as the indices into the
    // glTF nodes array

    glb.gltf["nodes"] = ctx.skel.tree.node_indices().map(|idx| {
        use petgraph::Direction;
        use skeleton::{Transform, SMatrix};
        let mut node = object!();

        let children = ctx.skel.tree
            .neighbors_directed(idx, Direction::Outgoing)
            .map(|child_idx| child_idx.index())
            .collect::<Vec<_>>();
        if !children.is_empty() {
            node["children"] = children.into();
        }

        match ctx.skel.tree[idx].local_to_parent {
            Transform::Root => {
                node["name"] = "<ROOT>".into();
            }
            Transform::SMatrix(SMatrix::Object { object_idx }) => {
                node["name"] = ctx.model
                    .objects[object_idx as usize]
                    .name
                    .to_string()
                    .into();
                let trs = &ctx.rest_trses.objects[object_idx as usize];
                if let Some(t) = trs.translation {
                    node["translation"] = array!(t.x, t.y, t.z);
                }
                if let Some(r) = trs.rotation_quaternion {
                    node["rotation"] = array!(r.v.x, r.v.y, r.v.z, r.s);
                }
                if let Some(s) = trs.scale {
                    node["scale"] = array!(s.x, s.y, s.z);
                }
            }
            Transform::SMatrix(SMatrix::InvBind { inv_bind_idx }) => {
                node["name"] = format!("<INV BIND #{}>", inv_bind_idx).into();
                // TODO
            }
            Transform::SMatrix(SMatrix::Uninitialized { stack_pos }) => {
                node["name"] = format!("<UNINITIALIZED #{}>", stack_pos).into();
            }
        }

        // Instantiate the mesh/skin at the root node
        if idx == ctx.skel.root {
            node["mesh"] = 0.into();
            node["skin"] = 0.into();
        }
        node
    }).collect::<Vec<_>>().into();

    // Make the skin

    let skel = &ctx.skel;

    let inv_bind_accessor = {
        let buf = glb.buffers.add(Buffer {
            alignment: 4,
            bytes: Vec::with_capacity(16 * skel.tree.node_count() * 4),
        });
        let dat = &mut glb.buffers[buf].bytes;
        for joint_idx in skel.tree.node_indices() {
            let joint = &skel.tree[joint_idx];
            let matrix: &[f64; 16] = joint.rest_world_to_local.as_ref();
            for &entry in matrix {
                dat.push_f32(entry as f32);
            }
        }
        let buf_view = glb.gltf["bufferViews"].add(object!(
            "buffer" => buf,
            "byteLength" => dat.len(),
        ));
        glb.gltf["accessors"].add(object!(
            "bufferView" => buf_view,
            "type" => "MAT4",
            "componentType" => FLOAT,
            "count" => skel.tree.node_count(),
        ))
    };

    glb.gltf["skins"] = array!(
        object!(
            "skeleton" => skel.root.index(),
            "joints" => (0..skel.tree.node_count()).collect::<Vec<_>>(),
            "inverseBindMatrices" => inv_bind_accessor,
        )
    );

    // Make a scene
    glb.gltf["scenes"] = array!(object!("nodes" => array!(skel.root.index())));
    glb.gltf["scene"] = 0.into();
}

fn materials(ctx: &Ctx, glb: &mut Glb) {
    let materials = ctx.model.materials.iter().map(|material| {
        object!(
            "name" => material.name.to_string(),
        )
    }).collect::<Vec<JsonValue>>();
    glb.gltf["materials"] = materials.into();
}
