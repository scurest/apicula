#[macro_use]
mod xml;
mod make_invertible;

use cgmath::{Matrix4, One};
use convert::image_namer::ImageNamer;
use db::{Database, ModelId};
use skeleton::{Skeleton, Transform, SMatrix};
use primitives::{self, Primitives, DynamicState};
use nitro::Model;
use petgraph::{Direction};
use petgraph::graph::NodeIndex;
use time;
use util::BiList;
use connection::Connection;
use self::xml::Xml;

static FRAME_LENGTH: f64 = 1.0 / 60.0; // 60 fps

struct Ctx<'a> {
    model_id: ModelId,
    model: &'a Model,
    db: &'a Database,
    conn: &'a Connection,
    image_namer: &'a ImageNamer,
    objects: &'a [Matrix4<f64>],
    prims: &'a Primitives,
    skel: &'a Skeleton,
}

pub fn write(
    db: &Database,
    conn: &Connection,
    image_namer: &ImageNamer,
    model_id: ModelId,
) -> String {
    let model = &db.models[model_id];

    // We need invertible matrices since we're obliged to give values for
    // inverse bind matrices.
    use self::make_invertible::make_invertible;
    let objects = &model.objects.iter()
        .map(|o| make_invertible(&o.matrix))
        .collect::<Vec<_>>();
    let uv_mats = &model.materials.iter()
        .map(|mat| {
            if mat.params.texcoord_transform_mode() == 1 {
                mat.texture_mat
            } else {
                Matrix4::one()
            }
        })
        .collect::<Vec<_>>();
    let state = DynamicState { objects, uv_mats };
    let prims = &Primitives::build(model, primitives::PolyType::TrisAndQuads, state);
    let skel = &Skeleton::build(model, objects);

    let ctx = Ctx { model_id, model, db, conn, image_namer, objects, prims, skel };

    let mut xml = Xml::with_capacity(1024 * 1024); // 1MiB

    xml!(xml;
        (r#"<?xml version="1.0" encoding="utf-8"?>"#);
        <COLLADA xmlns=["http://www.collada.org/2005/11/COLLADASchema"] version=["1.4.1"]>;
    );
    asset(&mut xml, &ctx);
    library_images(&mut xml, &ctx);
    library_materials(&mut xml, &ctx);
    library_effects(&mut xml, &ctx);
    if !ctx.prims.vertices.is_empty() {
        library_geometries(&mut xml, &ctx);
        library_controllers(&mut xml, &ctx);
        library_animations(&mut xml, &ctx);
        library_animation_clips(&mut xml, &ctx);
        library_visual_scenes(&mut xml, &ctx);
        scene(&mut xml, &ctx);
    }
    xml!(xml;
        /COLLADA>;
    );

    xml.string()
}

fn asset(xml: &mut Xml, _ctx: &Ctx) {
    let now = time::now_utc();
    let iso8601_datetime = time::strftime("%FT%TZ", &now).unwrap();
    xml!(xml;
        <asset>;
            <created>(iso8601_datetime)</created>;
            <modified>(iso8601_datetime)</modified>;
        /asset>;
    );
}

fn library_images(xml: &mut Xml, ctx: &Ctx) {
    use std::collections::HashSet;
    // Find the names for all the images this model uses
    let image_names = (0..ctx.model.materials.len())
        .filter_map(|material_id| {
            match ctx.conn.models[ctx.model_id].materials[material_id].image_id() {
                Ok(Some(image_id)) => Some(image_id),
                _ => None,
            }
        })
        .filter_map(|ids| ctx.image_namer.names.get(&ids))
        .collect::<HashSet<_>>();

    xml!(xml;
        <library_images>;
        for name in (image_names) {
            <image id=["image-"(name)]>;
                <init_from>(name)".png"</init_from>;
            /image>;
        }
        /library_images>;
    );
}

fn library_materials(xml: &mut Xml, ctx: &Ctx) {
    xml!(xml;
        <library_materials>;
        for (i, mat) in (ctx.model.materials.iter().enumerate()) {
            <material id=["material"(i)] name=[(mat.name.print_safe())]>;
                <instance_effect url=["#effect"(i)]/>;
            /material>;
        }
        /library_materials>;
    );
}

fn library_effects(xml: &mut Xml, ctx: &Ctx) {
    xml!(xml;
        <library_effects>;
    );
    for (material_id, mat) in ctx.model.materials.iter().enumerate() {
        let mat_conn = &ctx.conn.models[ctx.model_id].materials[material_id];
        let image_name = match mat_conn.image_id() {
            Ok(Some(image_id)) => ctx.image_namer.names.get(&image_id),
            _ => None,
        };

        xml!(xml;
            <effect id=["effect"(material_id)] name=[(mat.name.print_safe())]>;
                <profile_COMMON>;
        );

        if let Some(name) = image_name {
            let wrap = |repeat, mirror| {
                match (repeat, mirror) {
                    (false, _) => "CLAMP",
                    (true, false) => "WRAP",
                    (true, true) => "MIRROR",
                }
            };
            xml!(xml;
                <newparam sid=["Image-surface"]>;
                    <surface type=["2D"]>;
                        <init_from>"image-"(name)</init_from>;
                        <format>"A8R8G8B8"</format>;
                    /surface>;
                /newparam>;
                <newparam sid=["Image-sampler"]>;
                    <sampler2D>;
                        <source>"Image-surface"</source>;
                        <wrap_s>(wrap(mat.params.repeat_s(), mat.params.mirror_s()))</wrap_s>;
                        <wrap_t>(wrap(mat.params.repeat_t(), mat.params.mirror_t()))</wrap_t>;
                        <minfilter>"NEAREST"</minfilter>;
                        <magfilter>"NEAREST"</magfilter>;
                        <mipfilter>"NEAREST"</mipfilter>;
                    /sampler2D>;
                /newparam>;
            );
        }

        // Lookup the texture we're using and find out if it has transparency.
        let has_transparency = match mat_conn.image_id() {
            Ok(Some(image_id)) => {
                let texture_id = image_id.0;
                let params = ctx.db.textures[texture_id].params;
                let alpha_type = params.format().alpha_type(params);

                use nds::Alpha;
                match alpha_type {
                    Alpha::Opaque => false,
                    Alpha::Transparent | Alpha::Translucent => true,
                }
            }
            _ => false,
        };
        xml!(xml;
            <technique sid=["common"]>;
                <phong>;
                    <emission>;
                        <color>(mat.emission[0])" "(mat.emission[1])" "(mat.emission[2])" 1"</color>;
                    /emission>;
                    <ambient>;
                        <color>(mat.ambient[0])" "(mat.ambient[1])" "(mat.ambient[2])" 1"</color>;
                    /ambient>;
                    <diffuse>;
                    if (image_name.is_some()) {
                        <texture texture=["Image-sampler"] texcoord=["tc"]/>;
                    } else {
                        <color>(mat.diffuse[0])" "(mat.diffuse[1])" "(mat.diffuse[2])" "(mat.alpha)</color>;
                    }
                    /diffuse>;
                    <specular>;
                        <color>(mat.specular[0])" "(mat.specular[1])" "(mat.specular[2])" 1"</color>;
                    /specular>;
                    if (has_transparency) {
                        <transparent>;
                            <texture texture=["Image-sampler"] texcoord=["tc"]/>;
                        /transparent>;
                    }
                    if (mat.alpha != 1.0) {
                        <transparency>;
                            <float>(mat.alpha)</float>;
                        /transparency>;
                    }
                /phong>;
            /technique>;
        );

        xml!(xml;
                /profile_COMMON>;
            /effect>;
        );
    }
    xml!(xml;
        /library_effects>;
    );
}

fn library_geometries(xml: &mut Xml, ctx: &Ctx) {
    let model_name = ctx.model.name;
    xml!(xml;
        <library_geometries>;
            <geometry id=["geometry"] name=[(model_name.print_safe())]>;
                <mesh>;
    );

    let verts = &ctx.prims.vertices;

    // Positions
    xml!(xml;
        <source id=[("positions")]>;
            <float_array id=[("positions-array")] count=[(3 * verts.len())]>
            for v in (verts) {
                (v.position[0])" "(v.position[1])" "(v.position[2])" "
            }
            </float_array>;
            <technique_common>;
                <accessor source=["#positions-array"] count=[(verts.len())] stride=["3"]>;
                    <param name=["X"] type=["float"]/>;
                    <param name=["Y"] type=["float"]/>;
                    <param name=["Z"] type=["float"]/>;
                /accessor>;
            /technique_common>;
        /source>;
    );

    // Texcoords
    let has_texcoords = ctx.prims.draw_calls.iter().any(|call| call.used_texcoords);
    if has_texcoords {
        xml!(xml;
            <source id=["texcoords"]>;
                <float_array id=["texcoords-array"] count=[(2 * verts.len())]>
                for v in (verts) {
                    (v.texcoord[0])" "(v.texcoord[1])" "
                }
                </float_array>;
                <technique_common>;
                    <accessor source=["#texcoords-array"] count=[(verts.len())] stride=["2"]>;
                        <param name=["S"] type=["float"]/>;
                        <param name=["T"] type=["float"]/>;
                    /accessor>;
                /technique_common>;
            /source>;
        );
    }

    // Vertex colors
    let has_colors = ctx.prims.draw_calls.iter().any(|call| call.used_vertex_color);
    if has_colors {
        xml!(xml;
            <source id=["colors"]>;
                <float_array id=["colors-array"] count=[(3 * verts.len())]>
                for v in (verts) {
                    (v.color[0])" "(v.color[1])" "(v.color[2])" "
                }
                </float_array>;
                <technique_common>;
                    <accessor source=["#colors-array"] count=[(verts.len())] stride=["3"]>;
                        <param name=["R"] type=["float"]/>;
                        <param name=["G"] type=["float"]/>;
                        <param name=["B"] type=["float"]/>;
                    /accessor>;
                /technique_common>;
            /source>;
        );
    }

    // Normals
    let has_normals = ctx.prims.draw_calls.iter().any(|call| call.used_normals);
    if has_normals {
        xml!(xml;
            <source id=["normals"]>;
                <float_array id=["normals-array"] count=[(3 * verts.len())]>
                for v in (verts) {
                    (v.normal[0])" "(v.normal[1])" "(v.normal[2])" "
                }
                </float_array>;
                <technique_common>;
                    <accessor source=["#normals-array"] count=[(verts.len())] stride=["3"]>;
                        <param name=["X"] type=["float"]/>;
                        <param name=["Y"] type=["float"]/>;
                        <param name=["Z"] type=["float"]/>;
                    /accessor>;
                /technique_common>;
            /source>;
        );
    }

    xml!(xml;
        <vertices id=["vertices"]>;
            <input semantic=["POSITION"] source=["#positions"]/>;
            if (has_texcoords) {
            <input semantic=["TEXCOORD"] source=["#texcoords"]/>;
            }
            if (has_colors) {
            <input semantic=["COLOR"] source=["#colors"]/>;
            }
            if (has_normals) {
            <input semantic=["NORMAL"] source=["#normals"]/>;
            }
        /vertices>;
    );

    // One <polylist> per draw call
    for call in &ctx.prims.draw_calls {
        let indices = &ctx.prims.indices[call.index_range.clone()];
        // Remember indices come in groups of four.
        // [a, b, c, 0xffff] = triangle(a, b, c)
        // [a, b, c, d] = quad(a, b, c, d)
        let num_polys = indices.len() / 4;
        xml!(xml;
            <polylist material=["material"(call.mat_id)] count=[(num_polys)]>;
                <input semantic=["VERTEX"] source=["#vertices"] offset=["0"]/>;
                <vcount>
                for inds in (indices.chunks(4)) {
                    if (inds[3] == 0xffff) { "3 " }
                    else { "4 " }
                }
                </vcount>;
                <p>
                for &ind in (indices) {
                    if (ind != 0xffff) { (ind)" " }
                }
                </p>;
            /polylist>;
        );
    }

    xml!(xml;
                /mesh>;
            /geometry>;
        /library_geometries>;
    );
}

fn library_controllers(xml: &mut Xml, ctx: &Ctx) {
    xml!(xml;
        <library_controllers>;
            <controller id=["controller"]>;
                <skin source=["#geometry"]>;
    );

    let num_joints = ctx.skel.tree.node_count();

    // XML IDs of the joint <node>s
    xml!(xml;
        <source id=["controller-joints"]>;
            <Name_array id=["controller-joints-array"] count=[(num_joints)]>
            for j in (ctx.skel.tree.node_indices()) {
                "joint"(j.index())" "
            }
            </Name_array>;
            <technique_common>;
                <accessor source=["#controller-joints-array"] count=[(num_joints)]>;
                    <param name=["JOINT"] type=["Name"]/>;
                /accessor>;
            /technique_common>;
        /source>;
    );

    // Inverse bind matrices (ie. rest world-to-locals)
    xml!(xml;
        <source id=["controller-bind-poses"]>;
            <float_array id=["controller-bind-poses-array"] count=[(16 * num_joints)]>
            for j in (ctx.skel.tree.node_indices()) {
                MATRIX(&ctx.skel.tree[j].rest_world_to_local)" "
            }
            </float_array>;
            <technique_common>;
                <accessor source=["#controller-bind-poses-array"] count=[(num_joints)] stride=["16"]>;
                    <param name=["TRANSFORM"] type=["float4x4"]/>;
                /accessor>;
            /technique_common>;
        /source>;
    );

    // We gives weights by first giving a list of all weights we're going to
    // use and then giving indices into the list with the vertices, so we start
    // by gathering all weights into a list. Since weights are floats, we can't
    // insert them into a HashMap directly, so we first encode them as a
    // fixed-point number. Remember to decode them when they come out!
    let mut weights_lut = BiList::new();
    let encode = |x: f32| (x * 4096.0) as u32;
    let decode = |x: u32| x as f64 / 4096.0;
    weights_lut.clear();
    for v in &ctx.skel.vertices {
        for influence in &v.influences {
            weights_lut.push(encode(influence.weight));
        }
    }
    // Here is the list of all weights.
    xml!(xml;
        <source id=["controller-weights"]>;
            <float_array id=["controller-weights-array"] count=[(weights_lut.len())]>
            for &weight in (weights_lut.iter()) {
                (decode(weight))" "
            }
            </float_array>;
            <technique_common>;
                <accessor source=["#controller-weights-array"] count=[(weights_lut.len())]>;
                    <param name=["WEIGHT"] type=["float"]/>;
                /accessor>;
            /technique_common>;
        /source>;
    );

    xml!(xml;
        <joints>;
            <input semantic=["JOINT"] source=["#controller-joints"]/>;
            <input semantic=["INV_BIND_MATRIX"] source=["#controller-bind-poses"]/>;
        /joints>;
    );

    let num_verts = ctx.skel.vertices.len();
    xml!(xml;
        <vertex_weights count=[(num_verts)]>;
            <input semantic=["JOINT"] source=["#controller-joints"] offset=["0"]/>;
            <input semantic=["WEIGHT"] source=["#controller-weights"] offset=["1"]/>;
            <vcount>
            for v in (&ctx.skel.vertices) {
                (v.influences.len())" "
            }
            </vcount>;
            <v>
            for v in (&ctx.skel.vertices) {
                for influence in (&v.influences) {
                    (influence.joint.index())" "
                    (weights_lut.index(&encode(influence.weight)))" "
                }
            }
            </v>;
        /vertex_weights>;
    );

    xml!(xml;
                /skin>;
            /controller>;
        /library_controllers>;
    );
}

fn library_animations(xml: &mut Xml, ctx: &Ctx) {
    let anims = &ctx.conn.models[ctx.model_id].animations;
    if anims.is_empty() { return; }

    xml!(xml;
        <library_animations>;
    );
    for &anim_id in anims {
        let anim = &ctx.db.animations[anim_id];
        let num_frames = anim.num_frames;

        for joint_id in ctx.skel.tree.node_indices() {
            let joint_index = joint_id.index();
            let joint = &ctx.skel.tree[joint_id];
            let object_id = match joint.local_to_parent {
                Transform::SMatrix(SMatrix::Object { object_idx }) => object_idx,
                _ => continue,
            };

            xml!(xml;
                <animation id=["anim"(anim_id)"-joint"(joint_index)]>;
            );

            // Time
            xml!(xml;
                <source id=["anim"(anim_id)"-joint"(joint_index)"-time"]>;
                    <float_array id=["anim"(anim_id)"-joint"(joint_index)"-time-array"] count=[(num_frames)]>
                    for frame in (0..num_frames) {
                        (frame as f64 * FRAME_LENGTH)" "
                    }
                    </float_array>;
                    <technique_common>;
                        <accessor source=["#anim"(anim_id)"-joint"(joint_index)"-time-array"] count=[(num_frames)]>;
                            <param name=["TIME"] type=["float"]/>;
                        /accessor>;
                    /technique_common>;
                /source>;
            );

            // Matrix
            xml!(xml;
                <source id=["anim"(anim_id)"-joint"(joint_index)"-matrix"]>;
                    <float_array id=["anim"(anim_id)"-joint"(joint_index)"-matrix-array"] count=[(16 * num_frames)]>
                    for frame in (0..num_frames) {
                        MATRIX(
                            &anim.objects_curves.get(object_id as usize)
                                .map(|trs| trs.sample_at(frame))
                                .unwrap_or_else(|| Matrix4::one())
                        )" "
                    }
                    </float_array>;
                    <technique_common>;
                        <accessor source=["#anim"(anim_id)"-joint"(joint_index)"-matrix-array"] count=[(num_frames)] stride=["16"]>;
                            <param name=["TRANSFORM"] type=["float4x4"]/>;
                        /accessor>;
                    /technique_common>;
                /source>;
            );

            // Interpolation (all LINEAR)
            xml!(xml;
                <source id=["anim"(anim_id)"-joint"(joint_index)"-interpolation"]>;
                    <Name_array id=["anim"(anim_id)"-joint"(joint_index)"-interpolation-array"] count=[(num_frames)]>
                    for _frame in (0..num_frames) {
                        "LINEAR "
                    }
                    </Name_array>;
                    <technique_common>;
                        <accessor source=["#anim"(anim_id)"-joint"(joint_index)"-interpolation-array"] count=[(num_frames)]>;
                            <param name=["INTERPOLATION"] type=["Name"]/>;
                        /accessor>;
                    /technique_common>;
                /source>;
            );

            xml!(xml;
                <sampler id=["anim"(anim_id)"-joint"(joint_index)"-sampler"]>;
                    <input semantic=["INPUT"] source=["#anim"(anim_id)"-joint"(joint_index)"-time"]/>;
                    <input semantic=["OUTPUT"] source=["#anim"(anim_id)"-joint"(joint_index)"-matrix"]/>;
                    <input semantic=["INTERPOLATION"] source=["#anim"(anim_id)"-joint"(joint_index)"-interpolation"]/>;
                /sampler>;
            );

            xml!(xml;
                <channel
                    source=["#anim"(anim_id)"-joint"(joint_index)"-sampler"]
                    target=["joint"(joint_index)"/transform"]/>;
            );

            xml!(xml;
                /animation>;
            );
        }
    }
    xml!(xml;
        /library_animations>;
    );
}


fn library_animation_clips(xml: &mut Xml, ctx: &Ctx) {
    let anims = &ctx.conn.models[ctx.model_id].animations;
    if anims.is_empty() { return ;}

    xml!(xml;
        <library_animation_clips>;
    );
    for &anim_id in anims {
        let anim = &ctx.db.animations[anim_id];
        assert!(anim.num_frames != 0);
        let end_time = (anim.num_frames - 1) as f64 * FRAME_LENGTH;

        xml!(xml;
            <animation_clip id=["anim"(anim_id)] name=[(anim.name.print_safe())] end=[(end_time)]>;
            for j in (ctx.skel.tree.node_indices()) {
                <instance_animation url=["#anim"(anim_id)"-joint"(j.index())]/>;
            }
            /animation_clip>;
        );
    }
    xml!(xml;
        /library_animation_clips>;
    );
}

fn library_visual_scenes(xml: &mut Xml, ctx: &Ctx) {
    let model_name = ctx.model.name;

    xml!(xml;
        <library_visual_scenes>;
            <visual_scene id=["scene0"] name=[(model_name.print_safe())]>;
    );

    joint_hierarchy(xml, ctx);

    xml!(xml;
        <node id=["node"] name=[(model_name.print_safe())] type=["NODE"]>;
            <instance_controller url=["#controller"]>;
                <skeleton>"#joint"(ctx.skel.root.index())</skeleton>;
                <bind_material>;
                    <technique_common>;
                    for i in (0..ctx.model.materials.len()) {
                        <instance_material symbol=["material"(i)] target=["#material"(i)]>;
                            <bind_vertex_input semantic=["tc"] input_semantic=["TEXCOORD"]/>;
                        /instance_material>;
                    }
                    /technique_common>;
                /bind_material>;
            /instance_controller>;
        /node>;
    );

    xml!(xml;
            /visual_scene>;
        /library_visual_scenes>;
    );
}

fn joint_hierarchy(xml: &mut Xml, ctx: &Ctx) {
    /// Write the name for a joint that will appear in DCC programs.
    fn joint_name(ctx: &Ctx, node: NodeIndex) -> String {
        match ctx.skel.tree[node].local_to_parent {
            Transform::Root =>
                format!("__ROOT__"),
            Transform::SMatrix(SMatrix::Object { object_idx }) =>
                format!("{}", ctx.model.objects[object_idx as usize].name.print_safe()),
            Transform::SMatrix(SMatrix::InvBind { inv_bind_idx }) =>
                format!("__INV_BIND{}", inv_bind_idx),
            Transform::SMatrix(SMatrix::Uninitialized { stack_pos }) =>
                format!("__UNINITIALIZED{}", stack_pos),
        }
    }

    // Recursive tree walker
    fn rec(xml: &mut Xml, ctx: &Ctx, node: NodeIndex) {
        let tree = &ctx.skel.tree;

        xml!(xml;
            <node
                id=["joint"(node.index())]
                sid=["joint"(node.index())]
                name=[(joint_name(ctx, node))]
                type=["JOINT"]>;
        );

        let mat = match tree[node].local_to_parent {
            Transform::Root =>
                Matrix4::one(),
            Transform::SMatrix(SMatrix::Object { object_idx }) =>
                ctx.objects[object_idx as usize],
            Transform::SMatrix(SMatrix::InvBind { inv_bind_idx }) =>
                ctx.model.inv_binds[inv_bind_idx as usize],
            Transform::SMatrix(SMatrix::Uninitialized { .. }) =>
                Matrix4::one(),
        };
        xml!(xml;
            <matrix sid=["transform"]>MATRIX(&mat)</matrix>;
        );

        let children = tree.neighbors_directed(node, Direction::Outgoing);
        for child in children {
            rec(xml, ctx, child);
        }

        xml!(xml;
            /node>;
        );
    }

    rec(xml, ctx, ctx.skel.root)
}

fn scene(xml: &mut Xml, _ctx: &Ctx) {
    xml!(xml;
        <scene>;
            <instance_visual_scene url=["#scene0"]/>;
        /scene>;
    );
}
