use cgmath::Matrix4;
use cgmath::One;
use convert::format::FnFmt;
use convert::format::Mat;
use errors::Result;
use geometry::build_with_joints as build_geometry;
use geometry::GeometryDataWithJoints as GeometryData;
use joint_builder::JointTree;
use joint_builder::LinCombTerm;
use joint_builder::Transform;
use nitro::jnt;
use nitro::jnt::Animation;
use nitro::mdl::Model;
use nitro::name;
use nitro::tex::texpal::TexPalPair;
use petgraph::Direction;
use petgraph::graph::NodeIndex;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt::Write;
use time;
use util::uniq::UniqueNamer;

static FRAME_LENGTH: f64 = 1.0 / 60.0; // 60 fps

type ImageNamer = UniqueNamer<TexPalPair>;

pub fn write<W: Write>(
    w: &mut W,
    model: &Model,
    anims: &[Animation],
    image_namer: &mut ImageNamer,
) -> Result<()> {
    let objects = model.objects.iter().map(|o| o.xform).collect::<Vec<_>>();
    let geom = build_geometry(model, &objects[..])?;

    write_lines!(w,
        r##"<?xml version="1.0" encoding="utf-8"?>"##,
        r##"<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1">"##;
    )?;
    write_asset(w)?;
    write_library_images(w, model, image_namer)?;
    write_library_materials(w, model)?;
    write_library_effects(w, model, image_namer)?;
    write_library_geometries(w, model, &geom)?;
    write_library_controllers(w, &geom)?;
    write_library_animations(w, model, anims, &geom)?;
    write_library_animation_clips(w, model, anims, &geom)?;
    write_library_visual_scenes(w, model, &geom)?;
    write_scene(w)?;
    write_lines!(w,
        r##"</COLLADA>"##;
    )?;
    Ok(())
}

fn write_asset<W: Write>(w: &mut W) -> Result<()> {
    let now = time::now_utc();
    let iso8601_datetime = time::strftime("%FT%TZ", &now)?;
    write_lines!(w,
        r##"  <asset>"##,
        r##"    <created>{time}</created>"##,
        r##"    <modified>{time}</modified>"##,
        r##"  </asset>"##;
        time = iso8601_datetime,
    )?;
    Ok(())
}

fn write_library_images<W: Write>(
    w: &mut W,
    model: &Model,
    image_namer: &mut ImageNamer,
) -> Result<()> {
    write_lines!(w,
        r##"  <library_images>"##;
    )?;
    for mat in &model.materials {
        let res = TexPalPair::from_material(mat)
            .map(|pair| image_namer.get_name(pair));
        let name = match res {
            Some(name) => name,
            None => continue,
        };
        write_lines!(w,
            r##"    <image id="{name}">"##,
            r##"      <init_from>{name}.png</init_from>"##,
            r##"    </image>"##;
            name = name,
        )?;
    }
    write_lines!(w,
        r##"  </library_images>"##;
    )?;
    Ok(())
}

fn write_library_materials<W: Write>(w: &mut W, model: &Model) -> Result<()> {
    write_lines!(w,
        r##"  <library_materials>"##;
    )?;
    for (i, mat) in model.materials.iter().enumerate() {
        write_lines!(w,
            r##"    <material id="material{i}" name="{name}">"##,
            r##"      <instance_effect url="#effect{i}"/>"##,
            r##"    </material>"##;
            i = i,
            name = name::IdFmt(&mat.name),
        )?;
    }
    write_lines!(w,
        r##"  </library_materials>"##;
    )?;
    Ok(())
}

fn write_library_effects<W: Write>(
    w: &mut W,
    model: &Model,
    image_namer: &mut ImageNamer,
) -> Result<()> {
    write_lines!(w,
        r##"  <library_effects>"##;
    )?;
    for (i, mat) in model.materials.iter().enumerate() {
        let image_name = TexPalPair::from_material(mat)
            .map(|pair| image_namer.get_name(pair));

        write_lines!(w,
            r##"    <effect id="effect{i}" name="{name}">"##,
            r##"      <profile_COMMON>"##;
            i = i, name = name::IdFmt(&mat.name),
        )?;

        if let Some(name) = image_name {
            let wrap = |repeat, mirror| {
                match (repeat, mirror) {
                    (false, _) => "CLAMP",
                    (true, false) => "WRAP",
                    (true, true) => "MIRROR",
                }
            };
            write_lines!(w,
                r##"        <newparam sid="Image-surface">"##,
                r##"          <surface type="2D">"##,
                r##"            <init_from>{image_name}</init_from>"##,
                r##"            <format>A8R8G8B8</format>"##,
                r##"          </surface>"##,
                r##"        </newparam>"##,
                r##"        <newparam sid="Image-sampler">"##,
                r##"          <sampler2D>"##,
                r##"            <source>Image-surface</source>"##,
                r##"            <wrap_s>{wrap_s}</wrap_s>"##,
                r##"            <wrap_t>{wrap_t}</wrap_t>"##,
                r##"            <minfilter>NEAREST</minfilter>"##,
                r##"            <magfilter>NEAREST</magfilter>"##,
                r##"            <mipfilter>NEAREST</mipfilter>"##,
                r##"          </sampler2D>"##,
                r##"        </newparam>"##;
                image_name = name,
                wrap_s = wrap(mat.params.repeat_s(), mat.params.mirror_s()),
                wrap_t = wrap(mat.params.repeat_t(), mat.params.mirror_t()),
            )?;
        }

        write_lines!(w,
            r##"        <technique sid="common">"##,
            r##"          <phong>"##,
            r##"            <diffuse>"##,
            r##"              {diffuse}"##,
            r##"            </diffuse>"##,
            r##"            <transparent>"##,
            r##"              {transparent}"##,
            r##"            </transparent>"##,
            r##"          </phong>"##,
            r##"        </technique>"##;
            diffuse = match image_name {
                Some(_) => r#"<texture texture="Image-sampler" texcoord="tc"/>"#,
                None => r#"<color>1 1 1 1</color>"#,
            },
            transparent = match image_name {
                Some(_) => r#"<texture texture="Image-sampler" texcoord="tc"/>"#,
                None => r#"<color>0 0 0 1</color>"#,
            },
        )?;

        write_lines!(w,
            r##"      </profile_COMMON>"##,
            r##"    </effect>"##;
        )?;
    }
    write_lines!(w,
        r##"  </library_effects>"##;
    )?;
    Ok(())
}

fn write_library_geometries<W: Write>(w: &mut W, model: &Model, geom: &GeometryData) -> Result<()> {
    write_lines!(w,
        r##"  <library_geometries>"##;
    )?;

    for (i, call) in geom.draw_calls.iter().enumerate() {
        let mesh = &model.meshes[call.mesh_id as usize];
        let vert_range = call.vertex_range.start as usize .. call.vertex_range.end as usize;
        let verts = &geom.vertices[vert_range];

        write_lines!(w,
            r##"    <geometry id="geometry{i}" name="{name}">"##,
            r##"      <mesh>"##;
            i = i, name = name::IdFmt(&mesh.name),
        )?;

        write_lines!(w,
            r##"        <source id="geometry{i}-positions">"##,
            r##"          <float_array id="geometry{i}-positions-array" count="{num_floats}">{floats}</float_array>"##,
            r##"          <technique_common>"##,
            r##"            <accessor source="#geometry{i}-positions-array" count="{num_verts}" stride="3">"##,
            r##"              <param name="X" type="float"/>"##,
            r##"              <param name="Y" type="float"/>"##,
            r##"              <param name="Z" type="float"/>"##,
            r##"            </accessor>"##,
            r##"          </technique_common>"##,
            r##"        </source>"##;
            i = i, num_floats = 3 * verts.len(), num_verts = verts.len(),
            floats = FnFmt(|f| {
                for x in verts.iter().flat_map(|v| v.position.iter()) {
                    write!(f, "{} ", x)?;
                }
                Ok(())
            }),
        )?;

        write_lines!(w,
            r##"        <source id="geometry{i}-texcoords">"##,
            r##"          <float_array id="geometry{i}-texcoords-array" count="{num_floats}">{floats}</float_array>"##,
            r##"          <technique_common>"##,
            r##"            <accessor source="#geometry{i}-texcoords-array" count="{num_verts}" stride="2">"##,
            r##"              <param name="S" type="float"/>"##,
            r##"              <param name="T" type="float"/>"##,
            r##"            </accessor>"##,
            r##"          </technique_common>"##,
            r##"        </source>"##;
            i = i, num_floats = 2 * verts.len(), num_verts = verts.len(),
            floats = FnFmt(|f| {
                for x in verts.iter().flat_map(|v| v.texcoord.iter()) {
                    write!(f, "{} ", x)?;
                }
                Ok(())
            }),
        )?;

        // Omit the colors if they are all white
        let omit_colors = verts.iter().all(|v| v.color == [1.0, 1.0, 1.0]);
        if !omit_colors {
            write_lines!(w,
                r##"        <source id="geometry{i}-colors">"##,
                r##"          <float_array id="geometry{i}-colors-array" count="{num_floats}">{floats}</float_array>"##,
                r##"          <technique_common>"##,
                r##"            <accessor source="#geometry{i}-colors-array" count="{num_verts}" stride="3">"##,
                r##"              <param name="R" type="float"/>"##,
                r##"              <param name="G" type="float"/>"##,
                r##"              <param name="B" type="float"/>"##,
                r##"            </accessor>"##,
                r##"          </technique_common>"##,
                r##"        </source>"##;
                i = i, num_floats = 3 * verts.len(), num_verts = verts.len(),
                floats = FnFmt(|f| {
                    for x in verts.iter().flat_map(|v| v.color.iter()) {
                        write!(f, "{} ", x)?;
                    }
                    Ok(())
                }),
            )?;
        }

        write_lines!(w,
            r##"        <vertices id="geometry{i}-vertices">"##,
            r##"          <input semantic="POSITION" source="#geometry{i}-positions"/>"##,
            r##"          <input semantic="TEXCOORD" source="#geometry{i}-texcoords"/>"##;
            i = i,
        )?;
        if !omit_colors {
            write_lines!(w,
                r##"          <input semantic="COLOR" source="#geometry{i}-colors"/>"##;
                i = i,
            )?;
        }
        write_lines!(w,
            r#"        </vertices>"#;
        )?;

        let num_tris = (call.index_range.end - call.index_range.start) / 3;
        let start_index = call.vertex_range.start;
        write_lines!(w,
            r##"        <triangles material="material{mat_id}" count="{num_tris}">"##,
            r##"          <input semantic="VERTEX" source="#geometry{i}-vertices" offset="0"/>"##,
            r##"          <p>{indices}</p>"##,
            r##"        </triangles>"##;
            i = i, mat_id = call.mat_id, num_tris = num_tris,
            indices = FnFmt(|f| {
                for index in &geom.indices[call.index_range.clone()] {
                    // The indices in geom are counting from the first vertex
                    // in the whole model, but we want them relative to the
                    // start of just this <mesh>.
                    let index = index - start_index;
                    write!(f, "{} ", index)?;
                }
                Ok(())
            }),
        )?;

        write_lines!(w,
            r##"      </mesh>"##,
            r##"    </geometry>"##;
        )?;
    }

    write_lines!(w,
        r##"  </library_geometries>"##;
    )?;

    Ok(())
}

fn write_library_controllers<W: Write>(w: &mut W, geom: &GeometryData) -> Result<()> {
    write_lines!(w,
        r##"  <library_controllers>"##;
    )?;

    for (i, call) in geom.draw_calls.iter().enumerate() {
        write_lines!(w,
            r##"    <controller id="controller{i}">"##,
            r##"      <skin source="#geometry{i}">"##;
            i = i,
        )?;

        // Every vertex blends between multiple joints with different weights.
        // However, in typical COLLADA fashion, we can't specify the weights
        // directly. We need to build a list of all weights and reference them
        // by index into this list.
        //
        // We need to be able to look up both a weight from an index and an
        // index from a weight (see below). Hence the two maps.
        //
        // One more thing: we can't use floats as the key to a HashMap, so we
        // scale them up and truncate ((w * 512.0) as u32). You must remember to
        // invert this operation when you read a value back out.
        let mut weight_to_index = HashMap::new();
        let mut index_to_weight = HashMap::new();

        let vrange = call.vertex_range.start as usize..call.vertex_range.end as usize;
        for j in &geom.joint_data.vertices[vrange.clone()] {
            for &LinCombTerm { weight, .. } in &j.0 {
                let w = (weight * 512.0) as u32;
                match weight_to_index.entry(w) {
                    Entry::Vacant(v) => {
                        let n = index_to_weight.len();
                        v.insert(n);
                        index_to_weight.insert(n, w);
                    }
                    _ => (),
                }
            }
        }

        let num_joints = geom.joint_data.tree.node_count();

        // This data (names and inverse binds) should be shared by all the controllers,
        // say, by putting it all in the first controller and referencing the sources or
        // arrays from all the others. Blender does not seem to like this, so I duplicate
        // it.
        // TODO: Invetigate this more.

        write_lines!(w,
            r##"        <source id="controller{i}-joints">"##,
            r##"          <Name_array id="controller{i}-joints-array" count="{num_joints}">{joints}</Name_array>"##,
            r##"          <technique_common>"##,
            r##"            <accessor source="#controller{i}-joints-array" count="{num_joints}">"##,
            r##"              <param name="JOINT" type="Name"/>"##,
            r##"            </accessor>"##,
            r##"          </technique_common>"##,
            r##"        </source>"##;
            i = i, num_joints = num_joints,
            joints = FnFmt(|f| {
                for j in 0..num_joints {
                    write!(f, "joint{} ", j)?;
                }
                Ok(())
            }),
        )?;

        write_lines!(w,
            r##"        <source id="controller{i}-bind_poses">"##,
            r##"          <float_array id="controller{i}-bind_poses-array" count="{num_floats}">{floats}</float_array>"##,
            r##"          <technique_common>"##,
            r##"            <accessor source="#controller{i}-bind_poses-array" count="{num_joints}" stride="16">"##,
            r##"              <param name="TRANSFORM" type="float4x4"/>"##,
            r##"            </accessor>"##,
            r##"          </technique_common>"##,
            r##"        </source>"##;
            i = i, num_floats = 16 * num_joints, num_joints = num_joints,
            floats = FnFmt(|f| {
                for j in 0..num_joints {
                    let inv_bind = geom.joint_data.tree[NodeIndex::new(j)].inv_bind_matrix;
                    write!(f, "{} ", Mat(&inv_bind))?;
                }
                Ok(())
            }),
        )?;

        write_lines!(w,
            r##"        <source id="controller{i}-weights">"##,
            r##"          <float_array id="controller{i}-weights-array" count="{num_weights}">{weights}</float_array>"##,
            r##"          <technique_common>"##,
            r##"            <accessor source="#controller{i}-weights-array" count="1">"##,
            r##"              <param name="WEIGHT" type="float"/>"##,
            r##"            </accessor>"##,
            r##"          </technique_common>"##,
            r##"        </source>"##;
            i = i, num_weights = weight_to_index.len(),
            weights = FnFmt(|f| {
                for i in 0..index_to_weight.len() {
                    write!(f, "{} ", index_to_weight[&i] as f64 / 512.0)?;
                }
                Ok(())
            })
        )?;

        write_lines!(w,
            r##"        <joints>"##,
            r##"          <input semantic="JOINT" source="#controller{i}-joints"/>"##,
            r##"          <input semantic="INV_BIND_MATRIX" source="#controller{i}-bind_poses"/>"##,
            r##"        </joints>"##;
            i = i,
        )?;

        let num_verts = vrange.end - vrange.start;
        write_lines!(w,
            r##"        <vertex_weights count="{num_verts}">"##,
            r##"          <input semantic="JOINT" source="#controller{i}-joints" offset="0"/>"##,
            r##"          <input semantic="WEIGHT" source="#controller{i}-weights" offset="1"/>"##,
            r##"          <vcount>{vcount}</vcount>"##,
            r##"          <v>{v}</v>"##,
            r##"        </vertex_weights>"##;
            i = i, num_verts = num_verts,
            vcount = FnFmt(|f| {
                for j in &geom.joint_data.vertices[vrange.clone()] {
                    write!(f, "{} ", j.0.len())?;
                }
                Ok(())
            }),
            v = FnFmt(|f| {
                for j in &geom.joint_data.vertices[vrange.clone()] {
                for &LinCombTerm { weight, joint_id } in &j.0 {
                        write!(f, "{} {} ",
                            joint_id.index(),
                            weight_to_index[&((weight * 512.0) as u32)],
                        )?;
                    }
                }
                Ok(())
            }),
        )?;

        write_lines!(w,
            r##"      </skin>"##,
            r##"    </controller>"##;
        )?;
    }

    write_lines!(w,
        r##"  </library_controllers>"##;
    )?;

    Ok(())
}

fn write_library_animations<W: Write>(w: &mut W, model: &Model, anims: &[Animation], geom: &GeometryData) -> Result<()> {
    write_lines!(w,
        r##"  <library_animations>"##;
    )?;

    let num_objects = model.objects.len();
    let matching_anims = anims.iter().enumerate()
        .filter(|&(_, ref a)| a.objects.len() == num_objects);
    let num_joints = geom.joint_data.tree.node_count();

    for (anim_id, anim) in matching_anims {
        let num_frames = anim.num_frames;

        for joint_id in  0..num_joints {
            let joint = &geom.joint_data.tree[NodeIndex::new(joint_id)];
            let object_id = match joint.transform {
                Transform::Object(id) => id,
                _ => continue,
            };
            let object = &anim.objects[object_id as usize];

            write_lines!(w,
                r##"    <animation id="anim{anim_id}-joint{joint_id}">"##;
                anim_id = anim_id, joint_id = joint_id,
            )?;

            write_lines!(w,
                r##"      <source id="anim{anim_id}-joint{joint_id}-time">"##,
                r##"        <float_array id="anim{anim_id}-joint{joint_id}-time-array" count="{num_frames}">{times}</float_array>"##,
                r##"        <technique_common>"##,
                r##"          <accessor source="#anim{anim_id}-joint{joint_id}-time-array" count="{num_frames}">"##,
                r##"            <param name="TIME" type="float"/>"##,
                r##"          </accessor>"##,
                r##"        </technique_common>"##,
                r##"      </source>"##;
                anim_id = anim_id, joint_id = joint_id, num_frames = num_frames,
                times = FnFmt(|f| {
                    for frame in 0..num_frames {
                        write!(f, "{} ", frame as f64 * FRAME_LENGTH)?;
                    }
                    Ok(())
                }),
            )?;

            write_lines!(w,
                r##"      <source id="anim{anim_id}-joint{joint_id}-matrix">"##;
                anim_id = anim_id, joint_id = joint_id,
            )?;
            write!(w,
                r##"        <float_array id="anim{anim_id}-joint{joint_id}-matrix-array" count="{num_floats}">"##,
                anim_id = anim_id, joint_id = joint_id, num_floats = 16 * num_frames,
            )?;
            for frame in 0..num_frames {
                // The reason we can't do this with a FnFmt like the others is that getting the matrix
                // can result in an Error, and that Error can't "go through" a fmt::Result, so this has
                // to be directly contained in this function.
                let mat = jnt::object::to_matrix(object, anim, frame)?;
                write!(w, "{} ", Mat(&mat))?;
            }
            write!(w, "</float_array>\n")?;
            write_lines!(w,
                r##"        <technique_common>"##,
                r##"          <accessor source="#anim{anim_id}-joint{joint_id}-matrix-array" count="{num_frames}" stride="16">"##,
                r##"            <param name="TRANSFORM" type="float4x4"/>"##,
                r##"          </accessor>"##,
                r##"        </technique_common>"##,
                r##"      </source>"##;
                anim_id = anim_id, joint_id = joint_id, num_frames = num_frames,
            )?;

            write_lines!(w,
                r##"      <source id="anim{anim_id}-joint{joint_id}-interpolation">"##,
                r##"        <Name_array id="anim{anim_id}-joint{joint_id}-interpolation-array" count="{num_frames}">{interps}</Name_array>"##,
                r##"        <technique_common>"##,
                r##"          <accessor source="#anim{anim_id}-joint{joint_id}-interpolation-array" count="{num_frames}">"##,
                r##"            <param name="INTERPOLATION" type="name"/>"##,
                r##"          </accessor>"##,
                r##"        </technique_common>"##,
                r##"      </source>"##;
                anim_id = anim_id, joint_id = joint_id, num_frames = num_frames,
                interps = FnFmt(|f| {
                    for _ in 0..num_frames {
                        write!(f, "LINEAR ")?;
                    }
                    Ok(())
                }),
            )?;

            write_lines!(w,
                r##"      <sampler id="anim{anim_id}-joint{joint_id}-sampler">"##,
                r##"        <input semantic="INPUT" source="#anim{anim_id}-joint{joint_id}-time"/>"##,
                r##"        <input semantic="OUTPUT" source="#anim{anim_id}-joint{joint_id}-matrix"/>"##,
                r##"        <input semantic="INTERPOLATION" source="#anim{anim_id}-joint{joint_id}-interpolation"/>"##,
                r##"      </sampler>"##;
                anim_id = anim_id, joint_id = joint_id,
            )?;

            write_lines!(w,
                r##"      <channel source="#anim{anim_id}-joint{joint_id}-sampler" target="joint{joint_id}/transform"/>"##;
                anim_id = anim_id, joint_id = joint_id,
            )?;

            write_lines!(w,
                r##"    </animation>"##;
            )?;
        }
    }

    write_lines!(w,
        r##"  </library_animations>"##;
    )?;

    Ok(())
}


fn write_library_animation_clips<W: Write>(w: &mut W, model: &Model, anims: &[Animation], geom: &GeometryData) -> Result<()> {
    write_lines!(w,
        r##"  <library_animation_clips>"##;
    )?;

    let num_objects = model.objects.len();
    let matching_anims = anims.iter().enumerate()
        .filter(|&(_, ref a)| a.objects.len() == num_objects);
    let num_joints = geom.joint_data.tree.node_count();

    for (anim_id, anim) in matching_anims {
        check!(anim.num_frames != 0);
        let end_time = (anim.num_frames - 1) as f64 * FRAME_LENGTH;

        write_lines!(w,
            r##"    <animation_clip id="anim{anim_id}" name="{name}" end="{end_time}">"##;
            anim_id = anim_id, name = name::IdFmt(&anim.name), end_time = end_time,
        )?;
        for joint_id in  0..num_joints {
            write_lines!(w,
                r##"      <instance_animation url="#anim{anim_id}-joint{joint_id}"/>"##;
                anim_id = anim_id, joint_id = joint_id,
            )?;
        }
        write_lines!(w,
            r##"    </animation_clip>"##;
        )?;
    }

    write_lines!(w,
        r##"  </library_animation_clips>"##;
    )?;

    Ok(())
}

fn write_library_visual_scenes<W: Write>(w: &mut W, model: &Model, geom: &GeometryData) -> Result<()> {
    write_lines!(w,
        r#"  <library_visual_scenes>"#,
        r#"    <visual_scene id="scene0" name="{model_name}">"#;
        model_name = name::IdFmt(&model.name),
    )?;

    write_joint_heirarchy(w, model, geom)?;

    for (i, call) in geom.draw_calls.iter().enumerate() {
        let mesh = &model.meshes[call.mesh_id as usize];
        write_lines!(w,
            r##"      <node id="node{i}" name="{mesh_name}" type="NODE">"##,
            r##"        <instance_controller url="#controller{i}">"##,
            r##"          <skeleton>#joint0</skeleton>"##,
            r##"          <bind_material>"##,
            r##"            <technique_common>"##,
            r##"              <instance_material symbol="material{mat_id}" target="#material{mat_id}">"##;
            i = i, mesh_name = name::IdFmt(&mesh.name), mat_id = call.mat_id,
        )?;
        if model.materials[call.mat_id as usize].texture_name.is_some() {
            write_lines!(w,
                r##"                <bind_vertex_input semantic="tc" input_semantic="TEXCOORD"/>"##;
            )?;
        }
        write_lines!(w,
            r##"              </instance_material>"##,
            r##"            </technique_common>"##,
            r##"          </bind_material>"##,
            r##"        </instance_controller>"##,
            r##"      </node>"##;
        )?;
    }

    write_lines!(w,
        r##"    </visual_scene>"##,
        r##"  </library_visual_scenes>"##;
    )?;

    Ok(())
}

fn write_joint_heirarchy<W: Write>(w: &mut W, model: &Model, geom: &GeometryData) -> Result<()> {
    fn write_indent<W: Write>(w: &mut W, indent: u32) -> Result<()> {
        // Base indent
        write!(w, "      ")?;
        for _ in 0..indent {
            write!(w, "  ")?;
        }
        Ok(())
    }

    fn write<W: Write>(w: &mut W, model: &Model, tree: &JointTree, node: NodeIndex, indent: u32) -> Result<()> {
        write_indent(w, indent)?;
        write_lines!(w,
            r#"<node id="joint{joint_id}" sid="joint{joint_id}" name="{name}" type="JOINT">"#;
            joint_id = node.index(),
            name = FnFmt(|f| match tree[node].transform {
                Transform::Root => write!(f, "__ROOT__"),
                Transform::Object(id) => {
                    let object = &model.objects[id as usize];
                    write!(f, "{}", name::IdFmt(&object.name))
                }
                Transform::BlendDummy(id) => write!(f, "__BLEND{}__", id),
                Transform::UnknownStackSlot(pos) => write!(f, r"__STACK{}__", pos),
            }),
        )?;

        let mat = match tree[node].transform {
            Transform::Object(id) => model.objects[id as usize].xform,
            Transform::BlendDummy(id) => model.blend_matrices[id as usize].0,
            _ => Matrix4::one(),
        };
        write_indent(w, indent + 1)?;
        write_lines!(w, r#"<matrix sid="transform">{}</matrix>"#; Mat(&mat))?;

        let children = tree.neighbors_directed(node, Direction::Outgoing);
        for child in children {
            write(w, model, tree, child, indent + 1)?;
        }

        write_indent(w, indent)?;
        write!(w, "</node>\n")?;
        Ok(())
    }
    write(w, model, &geom.joint_data.tree, geom.joint_data.root, 0)
}

fn write_scene<W: Write>(w: &mut W) -> Result<()> {
    write_lines!(w,
        r##"  <scene>"##,
        r##"    <instance_visual_scene url="#scene0"/>"##,
        r##"  </scene>"##;
    )?;
    Ok(())
}
