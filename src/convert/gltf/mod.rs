mod gltf;
mod object_trs;
mod curve;
mod primitive;

use nitro::Model;
use db::{Database, ModelId};
use connection::Connection;
use primitives::{Primitives, PolyType, DynamicState};
use skeleton::{Skeleton, Transform, SMatrix};
use super::image_namer::ImageNamer;
use cgmath::{Matrix4, One};
use json::JsonValue;
use self::gltf::{GlTF, Buffer, ByteVec, VecExt};
use self::object_trs::ObjectTRSes;
use util::{BiVec, BiMap};
use self::curve::{GlTFObjectCurves, CurveDomain};
use nitro::animation::Curve;
use std::collections::HashMap;
use self::primitive::encode_ngons;
use nds::Alpha;

static FRAME_LENGTH: f32 = 1.0 / 60.0; // 60 fps

struct Ctx<'a> {
    model_id: ModelId,
    model: &'a Model,
    db: &'a Database,
    conn: &'a Connection,
    image_namer: &'a ImageNamer,
    rest_trses: ObjectTRSes,
    prims: &'a Primitives,
    skel: &'a Skeleton,
}

pub fn to_gltf(
    db: &Database,
    conn: &Connection,
    image_namer: &ImageNamer,
    model_id: ModelId,
) -> GlTF {
    let model = &db.models[model_id];

    let rest_trses = ObjectTRSes::for_model_at_rest(model);
    let objects = rest_trses.objects.iter()
        .map(Matrix4::from)
        .collect::<Vec<_>>();
    let uv_mats = model.materials.iter()
        .map(|mat| {
            if mat.params.texcoord_transform_mode() == 1 {
                mat.texture_mat
            } else {
                Matrix4::one()
            }
        })
        .collect::<Vec<Matrix4<f64>>>();
    let state = DynamicState { objects: &objects, uv_mats: &uv_mats };
    let prims = Primitives::build(model, PolyType::TrisAndQuads, state);
    let prims = &encode_ngons(prims);
    let skel = &Skeleton::build(model, &objects);

    let ctx = Ctx { model_id, model, db, conn, image_namer, rest_trses, prims, skel };

    let mut gltf = GlTF::new();

    if !ctx.prims.vertices.is_empty() {
        mesh(&ctx, &mut gltf);
        nodes(&ctx, &mut gltf);
        animations(&ctx, &mut gltf);
    }
    materials(&ctx, &mut gltf);

    gltf.cleanup();
    gltf
}

// glTF constants
static UNSIGNED_BYTE: u32 = 5121;
static UNSIGNED_SHORT: u32 = 5123;
static FLOAT: u32 = 5126;
static NEAREST: u32 = 9728;

fn mesh(ctx: &Ctx, gltf: &mut GlTF) {
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
        let buf = gltf.buffers.add(Buffer {
            alignment: 4,
            bytes: Vec::with_capacity(3 * verts.len() * 4),
        });
        let dat = &mut gltf.buffers[buf].bytes;
        for v in verts {
            dat.push_f32(v.position[0]);
            dat.push_f32(v.position[1]);
            dat.push_f32(v.position[2]);
        }
        let buf_view = gltf.json["bufferViews"].add(object!(
            "buffer" => buf,
            "byteLength" => dat.len(),
        ));
        gltf.json["accessors"].add(object!(
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
        let buf = gltf.buffers.add(Buffer {
            alignment: 4,
            bytes: Vec::with_capacity(2 * verts.len() * 4),
        });
        let dat = &mut gltf.buffers[buf].bytes;
        for v in verts {
            dat.push_f32(v.texcoord[0]);
            dat.push_f32(1.0 - v.texcoord[1]);
        }
        let buf_view = gltf.json["bufferViews"].add(object!(
            "buffer" => buf,
            "byteLength" => dat.len(),
        ));
        Some(gltf.json["accessors"].add(object!(
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
        let buf = gltf.buffers.add(Buffer {
            alignment: 4,
            bytes: Vec::with_capacity(4 * verts.len() * 1),
        });
        let dat = &mut gltf.buffers[buf].bytes;
        // Is the DS in sRGB??
        fn srgb_to_linear(s: f32) -> f32 {
            if s < 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        }
        // Since each channel is originally only 5 bits, 8 bits should be enough
        // to store it in linear space, so use normalized u8s.
        for v in verts {
            dat.push_normalized_u8(srgb_to_linear(v.color[0]));
            dat.push_normalized_u8(srgb_to_linear(v.color[1]));
            dat.push_normalized_u8(srgb_to_linear(v.color[2]));
            dat.push(255); // padding
        }
        let buf_view = gltf.json["bufferViews"].add(object!(
            "buffer" => buf,
            "byteLength" => dat.len(),
            "byteStride" => 4,
        ));
        Some(gltf.json["accessors"].add(object!(
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
        let buf = gltf.buffers.add(Buffer {
            alignment: 4,
            bytes: Vec::with_capacity(3 * verts.len() * 4),
        });
        let dat = &mut gltf.buffers[buf].bytes;
        for v in verts {
            dat.push_f32(v.normal[0]);
            dat.push_f32(v.normal[1]);
            dat.push_f32(v.normal[2]);
        }
        let buf_view = gltf.json["bufferViews"].add(object!(
            "buffer" => buf,
            "byteLength" => dat.len(),
        ));
        Some(gltf.json["accessors"].add(object!(
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
        let buf = gltf.buffers.add(Buffer {
            alignment: 4,
            bytes: Vec::with_capacity(4 * num_sets * verts.len() * 1),
        });
        let dat_len = {
            let dat = &mut gltf.buffers[buf].bytes;
            for sv in &ctx.skel.vertices {
                let mut i = 0;
                while i != 4 * num_sets {
                    if i < sv.influences.len() {
                        dat.push(sv.influences[i].joint as u8);
                    } else {
                        dat.push(0);
                    }
                    i += 1;
                }
            }
            dat.len()
        };
        (0..num_sets).map(|set_num| {
            let buf_view = gltf.json["bufferViews"].add(object!(
                "buffer" => buf,
                "byteOffset" => 4 * set_num,
                "byteStride" => 4 * num_sets,
                "byteLength" => dat_len - 4 * set_num,
            ));
            gltf.json["accessors"].add(object!(
                "bufferView" => buf_view,
                "type" => "VEC4",
                "componentType" => UNSIGNED_BYTE,
                "count" => verts.len(),
            ))
        }).collect::<Vec<_>>()
    };

    // Weights
    let weights_accessors = {
        let buf = gltf.buffers.add(Buffer {
            alignment: 4,
            bytes: Vec::with_capacity(4 * num_sets * verts.len() * 1),
        });
        let dat_len = {
            let dat = &mut gltf.buffers[buf].bytes;
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
            let buf_view = gltf.json["bufferViews"].add(object!(
                "buffer" => buf,
                "byteOffset" => 4 * set_num,
                "byteStride" => 4 * num_sets,
                "byteLength" => dat_len - 4 * set_num,
            ));
            gltf.json["accessors"].add(object!(
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
        let buf = gltf.buffers.add(Buffer {
            alignment: 2,
            bytes: Vec::with_capacity(verts.len() * 2),
        });
        let dat = &mut gltf.buffers[buf].bytes;
        for &ind in &ctx.prims.indices {
            dat.push_u16(ind);
        }
        gltf.json["bufferViews"].add(object!(
            "buffer" => buf,
            "byteLength" => dat.len(),
        ))
    };

    // One glTF primitive per draw call
    let primitives = ctx.prims.draw_calls.iter().map(|call| {
        let indices_accessor = gltf.json["accessors"].add(object!(
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
            "extensions" => object!(
                "FB_ngon_encoding" => object!(),
            ),
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

    gltf.json["meshes"] = array!(
        object!(
            "primitives" => primitives,
            "name" => ctx.model.name.to_string(),
        )
    );

    gltf.json["extensionsUsed"].push("FB_ngon_encoding").unwrap();
}

fn nodes(ctx: &Ctx, gltf: &mut GlTF) {
    if ctx.prims.draw_calls.is_empty() {
        return;
    }

    // Make a node tree from the skeleton tree. The NodeIndices for skel.tree
    // are the same as the indices into the glTF nodes array.
    gltf.json["nodes"] = ctx.skel.tree.node_idxs().map(|idx| {
        let mut node = object!();

        let children = ctx.skel.tree
            .children(idx)
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

        node
    }).collect::<Vec<_>>().into();

    // Add another node above the skeleton root to instantiate the mesh at
    // (glTF-Blender-IO doesn't like it when we instantiate a mesh on a node
    // that's also used as a joint).
    gltf.json["nodes"].push(object!(
        "mesh" => 0,
        "skin" => 0,
        "name" => ctx.model.name.to_string(),
        "children" => array!(ctx.skel.root),
    )).unwrap();

    // Make the skin

    let skel = &ctx.skel;

    let inv_bind_accessor = {
        let buf = gltf.buffers.add(Buffer {
            alignment: 4,
            bytes: Vec::with_capacity(16 * skel.tree.node_count() * 4),
        });
        let dat = &mut gltf.buffers[buf].bytes;
        for joint_idx in skel.tree.node_idxs() {
            let joint = &skel.tree[joint_idx];
            let matrix: &[f64; 16] = joint.rest_world_to_local.as_ref();
            for &entry in matrix {
                dat.push_f32(entry as f32);
            }
        }
        let buf_view = gltf.json["bufferViews"].add(object!(
            "buffer" => buf,
            "byteLength" => dat.len(),
        ));
        gltf.json["accessors"].add(object!(
            "bufferView" => buf_view,
            "type" => "MAT4",
            "componentType" => FLOAT,
            "count" => skel.tree.node_count(),
        ))
    };

    gltf.json["skins"] = array!(
        object!(
            "skeleton" => skel.root,
            "joints" => (0..skel.tree.node_count()).collect::<Vec<_>>(),
            "inverseBindMatrices" => inv_bind_accessor,
        )
    );

    gltf.json["scenes"] = array!(
        object!(
            "nodes" => array!(skel.tree.node_count()),
            "name" => ctx.model.name.to_string(),
        )
    );
    gltf.json["scene"] = 0.into();
}

fn animations(ctx: &Ctx, gltf: &mut GlTF) {
    let models_animations = &ctx.conn.models[ctx.model_id].animations;
    let mat_animations = &ctx.conn.models[ctx.model_id].mat_anims;

    if models_animations.is_empty() && mat_animations.is_empty() {
        return;
    }

    #[derive(Hash, Copy, Clone, PartialEq, Eq)]
    struct TimelineDescriptor {
        start_frame: u16,
        end_frame: u16,
        sampling_rate: u16,
    }
    // Maps an accessor index to the description of the keyframes it should
    // contain. Let's us reuse keyframes between curves. We wait until we're
    // done writing the animations to actually fill in the accessor fields
    // though.
    let mut timeline_descs = BiMap::<usize, TimelineDescriptor>::new();

    // Used to hold the sample data.
    let data_buffer = gltf.buffers.add(Buffer {
        bytes: Vec::with_capacity(1024 * 512),
        alignment: 4,
    });
    let data_buf_view = gltf.json["bufferViews"].add(object!(
        "buffer" => data_buffer,
        // NOTE: must fill out byte length when we finish writing to data_buffer
    ));

    let mut animations =
        models_animations.iter()
        .map(|&animation_id| {
            let anim = &ctx.db.animations[animation_id];

            let object_curves =
                anim.objects_curves.iter()
                .map(|c| GlTFObjectCurves::for_trs_curves(c))
                .collect::<Vec<GlTFObjectCurves>>();

            #[derive(Hash, Clone, Copy, PartialEq, Eq)]
            enum SamplerPath {
                Translation,
                Rotation,
                Scale,
            }
            #[derive(Hash, Clone, Copy, PartialEq, Eq)]
            struct SamplerDescriptor {
                object_idx: u8,
                path: SamplerPath,
            }
            // Each glTF sampler will contain the curve for one TRS property of
            // one object matrix. This maps a sampler index to the description
            // of what it will contain.
            let mut sampler_descs = BiVec::<SamplerDescriptor>::new();

            // The channels array wires nodes/paths up to the samplers they use.
            let mut channels = Vec::<JsonValue>::new();

            for node_idx in ctx.skel.tree.node_idxs() {
                // Only objects are animated
                let object_idx = match ctx.skel.tree[node_idx].local_to_parent {
                    Transform::SMatrix(SMatrix::Object { object_idx }) => object_idx,
                    _ => continue,
                };

                let curves = &object_curves[object_idx as usize];

                // Add channels for any of the TRSs that are animated for this
                // object

                if let Curve::Samples { .. } = curves.translation {
                    let sampler_descriptor = SamplerDescriptor {
                        object_idx,
                        path: SamplerPath::Translation,
                    };
                    sampler_descs.push(sampler_descriptor);
                    channels.push(object!(
                        "target" => object!(
                            "node" => node_idx,
                            "path" => "translation",
                        ),
                        "sampler" => sampler_descs.idx(&sampler_descriptor),
                    ));
                }

                if let Curve::Samples { .. } = curves.rotation {
                    let sampler_descriptor = SamplerDescriptor {
                        object_idx,
                        path: SamplerPath::Rotation,
                    };
                    sampler_descs.push(sampler_descriptor);
                    channels.push(object!(
                        "target" => object!(
                            "node" => node_idx,
                            "path" => "rotation",
                        ),
                        "sampler" => sampler_descs.idx(&sampler_descriptor),
                    ));
                }

                if let Curve::Samples { .. } = curves.scale {
                    let sampler_descriptor = SamplerDescriptor {
                        object_idx,
                        path: SamplerPath::Scale,
                    };
                    sampler_descs.push(sampler_descriptor);
                    channels.push(object!(
                        "target" => object!(
                            "node" => node_idx,
                            "path" => "scale",
                        ),
                        "sampler" => sampler_descs.idx(&sampler_descriptor),
                    ));
                }
            }

            // Now use the sampler descriptions to write the actual samplers
            let samplers = sampler_descs.iter().map(|desc| {
                let &SamplerDescriptor { object_idx, path } = desc;
                let curves = &object_curves[object_idx as usize];

                let domain = match path {
                    SamplerPath::Translation => curves.translation.domain(),
                    SamplerPath::Rotation => curves.rotation.domain(),
                    SamplerPath::Scale => curves.scale.domain(),
                };
                let (start_frame, end_frame, sampling_rate) = match domain {
                    CurveDomain::None => unreachable!(),
                    CurveDomain::Sampled { start_frame, end_frame, sampling_rate } =>
                        (start_frame, end_frame, sampling_rate),
                };
                let timeline_descriptor = TimelineDescriptor {
                    start_frame, end_frame, sampling_rate,
                };

                // Reserve the input accessor
                if !timeline_descs.right_contains(&timeline_descriptor) {
                    let accessor = gltf.json["accessors"].add(
                        JsonValue::new_object()
                    );
                    timeline_descs.insert((accessor, timeline_descriptor));
                };
                let &input = timeline_descs.backward(&timeline_descriptor);

                // Make the output accessor
                let data = &mut gltf.buffers[data_buffer].bytes;
                let output = match path {
                    SamplerPath::Translation | SamplerPath::Scale => {
                        let values = match path {
                            SamplerPath::Translation => match curves.translation {
                                Curve::Samples { ref values, .. } => values,
                                _ => unreachable!(),
                            },
                            SamplerPath::Scale => match curves.scale {
                                Curve::Samples { ref values, .. } => values,
                                _ => unreachable!(),
                            },
                            _ => unreachable!(),
                        };
                        let byte_offset = data.len();
                        data.reserve(3 * values.len() * 4);
                        for v in values {
                            data.push_f32(v.x as f32);
                            data.push_f32(v.y as f32);
                            data.push_f32(v.z as f32);
                        }
                        gltf.json["accessors"].add(object!(
                            "bufferView" => data_buf_view,
                            "type" => "VEC3",
                            "componentType" => FLOAT,
                            "byteOffset" => byte_offset,
                            "count" => values.len(),
                        ))
                    }

                    SamplerPath::Rotation => {
                        let values = match curves.rotation {
                            Curve::Samples { ref values, .. } => values,
                            _ => unreachable!(),
                        };
                        let byte_offset = data.len();
                        data.reserve(4 * values.len() * 4);
                        for quat in values {
                            data.push_f32(quat.v.x as f32);
                            data.push_f32(quat.v.y as f32);
                            data.push_f32(quat.v.z as f32);
                            data.push_f32(quat.s as f32);
                        }
                        gltf.json["accessors"].add(object!(
                            "bufferView" => data_buf_view,
                            "type" => "VEC4",
                            "componentType" => FLOAT,
                            "byteOffset" => byte_offset,
                            "count" => values.len(),
                        ))
                        // IDEA: would probably be okay to emit normalized i16s
                        // instead of floats...
                    }
                };

                object!(
                    "input" => input,
                    "output" => output,
                    "interpolation" => "STEP",
                )
            }).collect::<Vec<JsonValue>>();

            object!(
                "name" => anim.name.to_string(),
                "samplers" => samplers,
                "channels" => channels,
            )
        })
        .collect::<Vec<JsonValue>>();

    // Now material animations
    let model = &ctx.db.models[ctx.model_id];
    let mut had_mat_anims = false;
    for mat_anim_conn in mat_animations {
        let mat_anim = &ctx.db.mat_anims[mat_anim_conn.mat_anim_id];

        let mut channels: Vec<JsonValue> = vec![];
        let mut samplers: Vec<JsonValue> = vec![];

        for track in &mat_anim.tracks {
            let u_off_curve = &track.channels[3].curve;
            let v_off_curve = &track.channels[4].curve;

            // Get common domain
            let domain = u_off_curve.domain().union(v_off_curve.domain());
            let (start_frame, end_frame, sampling_rate) = match domain {
                CurveDomain::None => continue,
                CurveDomain::Sampled { start_frame, end_frame, sampling_rate } =>
                    (start_frame, end_frame, sampling_rate),
            };
            let timeline_descriptor = TimelineDescriptor {
                start_frame, end_frame, sampling_rate,
            };

            // Reserve the input accessor
            if !timeline_descs.right_contains(&timeline_descriptor) {
                let accessor = gltf.json["accessors"].add(
                    JsonValue::new_object()
                );
                timeline_descs.insert((accessor, timeline_descriptor));
            };
            let &input = timeline_descs.backward(&timeline_descriptor);

            // Find the target
            let material_idx = model.materials.iter().position(|mat| mat.name == track.name).unwrap();
            let target = format!(
                "/materials/{}/pbrMetallicRoughness/baseColorTexture/extensions/KHR_texture_transform/offset",
                material_idx,
            );

            // Find texture dimensions
            let (w, h) = (model.materials[material_idx].width as f64, model.materials[material_idx].height as f64);

            let data = &mut gltf.buffers[data_buffer].bytes;
            let byte_offset = data.len();
            let num_samples = (end_frame - start_frame) / sampling_rate ;
            data.reserve(4 * 2 * num_samples as usize);
            let mut frame = start_frame;
            while frame < end_frame {
                let u_off = u_off_curve.sample_at(0.0, frame);
                let v_off = v_off_curve.sample_at(0.0, frame);

                // Convert to glTF texture space
                let u_off = u_off / w;
                let v_off = v_off / h;

                data.push_f32(u_off as f32);
                data.push_f32(v_off as f32);

                frame += sampling_rate;
            }
            let output = gltf.json["accessors"].add(object!(
                "bufferView" => data_buf_view,
                "type" => "VEC2",
                "componentType" => FLOAT,
                "byteOffset" => byte_offset,
                "count" => num_samples,
            ));

            let sampler = samplers.add(object!(
                "input" => input,
                "output" => output,
                "interpolation" => "STEP",
            ));

            channels.push(object!(
                "target" => target,
                "sampler" => sampler,
            ));
        }

        if samplers.is_empty() { continue }

        animations.push(object!(
            "name" => mat_anim.name.to_string(),
            "samplers" => samplers,
            // glTF requires this be non-empty, so we add a channel that does
            // nothing.
            "channels" => vec![object!(
                "target" => object!("path" => "scale"),
                "sampler" => 0,
            )],
            "extensions" => object!(
                "EXT_property_animation" => object!(
                    "channels" => channels,
                )
            )
        ));

        had_mat_anims = true;
    }

    if had_mat_anims {
        gltf.json["extensionsUsed"].push("KHR_texture_transform").unwrap();
        gltf.json["extensionsUsed"].push("EXT_property_animation").unwrap();
    }

    gltf.json["bufferViews"][data_buf_view]["byteLength"] =
        gltf.buffers[data_buffer].bytes.len().into();

    // Now we need to write out the keyframe descriptors to real accessors.
    // The reason we deferred it is because we can share most of this data.
    //
    // For each rate, find the range of values used by timelines with that
    // rate. Write that range of values sampled at that rate into a buffer.
    // Eg.
    //
    //     rate 1:      1 2 3 4 5
    //     rate 2:      2 4
    //     rate 4:      4 8 12
    //
    // Make a buffer view for each of these rates. Then for each accessor,
    // reference the buffer view for that rate and use the byteOffset and
    // count properties to select the appropriate subrange.

    let mut rate_to_range = HashMap::<u16, std::ops::Range<u16>>::new();
    let mut rate_to_buf_view = HashMap::<u16, usize>::new();

    for (_, &timeline_desc) in timeline_descs.iter() {
        let TimelineDescriptor { start_frame, end_frame, sampling_rate } =
            timeline_desc;
        let range =
            rate_to_range.entry(sampling_rate)
            .or_insert(start_frame..end_frame);
        range.start = range.start.min(start_frame);
        range.end = range.end.max(end_frame);
    }

    let time_buf = gltf.buffers.add(Buffer {
        alignment: 4,
        bytes: vec![],
    });
    let dat = &mut gltf.buffers[time_buf].bytes;
    for (&rate, range) in rate_to_range.iter() {
        let byte_offset = dat.len();

        let mut frame = range.start;
        while frame < range.end {
            dat.push_f32(frame as f32 * FRAME_LENGTH);
            frame += rate;
        }

        let buf_view = gltf.json["bufferViews"].add(object!(
            "buffer" => time_buf,
            "byteOffset" => byte_offset,
            "byteLength" => dat.len() - byte_offset,
        ));

        rate_to_buf_view.insert(rate, buf_view);
    }

    for (&accessor_idx, &timeline_desc) in timeline_descs.iter() {
        let TimelineDescriptor { start_frame, end_frame, sampling_rate } =
            timeline_desc;

        let range = rate_to_range[&sampling_rate].clone();
        let buf_view = rate_to_buf_view[&sampling_rate];

        // The offset inside the buffer view of our starting frame
        let offset = (start_frame - range.start) / sampling_rate;
        let byte_offset = 4 * offset;

        let min = start_frame as f32 * FRAME_LENGTH;
        let max = (end_frame - sampling_rate) as f32 * FRAME_LENGTH;

        gltf.json["accessors"][accessor_idx] = object!(
            "bufferView" => buf_view,
            "type" => "SCALAR",
            "componentType" => FLOAT,
            "byteOffset" => byte_offset,
            "count" => (end_frame - start_frame) / sampling_rate,
            "min" => array!(min),
            "max" => array!(max),
        );
    }

    gltf.json["animations"] = animations.into();
}

fn materials(ctx: &Ctx, gltf: &mut GlTF) {
    #[derive(Copy, Clone, Hash, PartialEq, Eq)]
    enum WrapMode {
        Clamp,
        MirroredRepeat,
        Repeat,
    }
    #[derive(Copy, Clone, Hash, PartialEq, Eq)]
    struct SamplerDescriptor {
        wrap_s: WrapMode,
        wrap_t: WrapMode,
    }
    // Maps a sampler index to the wrapping mode it should use.
    let mut sampler_descs = BiVec::<SamplerDescriptor>::new();

    // Maps an image index to the image name it should use.
    let mut image_descs = BiVec::<String>::new();

    #[derive(Copy, Clone, Hash, PartialEq, Eq)]
    struct TextureDescriptor {
        sampler: usize,
        image: usize,
    }
    // Maps a texture index to the sampler and image it will use.
    let mut texture_descs = BiVec::<TextureDescriptor>::new();

    let materials = ctx.model.materials.iter().enumerate()
        .map(|(material_idx, material)| {
        let mut mat = object!(
            "name" => material.name.to_string(),
            "pbrMetallicRoughness" => JsonValue::new_object(),
            "extensions" => object!(
                "KHR_materials_unlit" => JsonValue::new_object(),
            )
        );

        let image_id =
            ctx.conn.models[ctx.model_id]
            .materials[material_idx].image_id();
        match image_id {
            Ok(Some(image_id)) => {
                let params = ctx.db.textures[image_id.0].params;
                match params.format().alpha_type(params) {
                    Alpha::Opaque => (),
                    Alpha::Transparent =>
                        mat["alphaMode"] = "MASK".into(),
                    Alpha::Translucent =>
                        mat["alphaMode"] = "BLEND".into(),
                }

                let wrap = |repeat, mirror| {
                    match (repeat, mirror) {
                        (false, _) => WrapMode::Clamp,
                        (true, false) => WrapMode::Repeat,
                        (true, true) => WrapMode::MirroredRepeat,
                    }
                };
                let params = material.params;
                let sampler_desc = SamplerDescriptor {
                    wrap_s: wrap(params.repeat_s(), params.mirror_s()),
                    wrap_t: wrap(params.repeat_t(), params.mirror_t()),
                };
                let sampler = sampler_descs.push(sampler_desc);

                let image_name = &ctx.image_namer.names[&image_id];
                let image = image_descs.push(image_name.clone());

                let texture_desc = TextureDescriptor { sampler, image };
                let texture = texture_descs.push(texture_desc);

                mat["pbrMetallicRoughness"]["baseColorTexture"] =
                    object!("index" => texture);
                mat["pbrMetallicRoughness"]["metallicFactor"] = 0.into();

            }
            _ => (),
        }

        let has_diffuse =
            !material.diffuse_is_default_vertex_color &&
            material.diffuse != [1.0, 1.0, 1.0];
        if has_diffuse || material.alpha != 1.0 {
            let [r, g, b] = if has_diffuse {
                material.diffuse
            } else {
                [1.0, 1.0, 1.0]
            };
            mat["pbrMetallicRoughness"]["baseColorFactor"] = array!(r, g, b, material.alpha);
        }

        if material.alpha == 0.0 {
            mat["alphaMode"] = "MASK".into();
        } else if material.alpha != 1.0 {
            mat["alphaMode"] = "BLEND".into();
        }

        if material.emission != [0.0, 0.0, 0.0] {
            // Does nothing since we use KHR_materials_unlit
            mat["emissiveFactor"] = material.emission.to_vec().into();
        }

        if !material.cull_backface {
            mat["doubleSided"] = true.into();
        }
        // TODO: handle cull frontfacing

        if mat["pbrMetallicRoughness"].is_empty() {
            mat.remove("pbrMetallicRoughness");
        }

        mat
    }).collect::<Vec<JsonValue>>();

    let wrap = |wrap_mode| {
        match wrap_mode {
            WrapMode::Clamp => 33071,
            WrapMode::MirroredRepeat => 33648,
            WrapMode::Repeat => 10497,
        }
    };
    gltf.json["samplers"] = sampler_descs.iter().map(|desc| {
        object!(
            "wrapS" => wrap(desc.wrap_s),
            "wrapT" => wrap(desc.wrap_t),
            "magFilter" => NEAREST,
            "minFilter" => NEAREST,
        )
    }).collect::<Vec<JsonValue>>().into();

    gltf.json["images"] = image_descs.iter().map(|name| {
        object!(
            "uri" => format!("{}.png", name),
        )
    }).collect::<Vec<JsonValue>>().into();

    gltf.json["textures"] = texture_descs.iter().map(|desc| {
        object!(
            "source" => desc.image,
            "sampler" => desc.sampler,
        )
    }).collect::<Vec<JsonValue>>().into();

    gltf.json["materials"] = materials.into();

    if gltf.json["samplers"].is_empty() { gltf.json.remove("samplers"); }
    if gltf.json["images"].is_empty() { gltf.json.remove("images"); }
    if gltf.json["textures"].is_empty() { gltf.json.remove("textures"); }
    if gltf.json["materials"].is_empty() { gltf.json.remove("materials"); }

    if gltf.json.has_key("materials") {
        gltf.json["extensionsUsed"].push("KHR_materials_unlit").unwrap();
    }
}
