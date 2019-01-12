use cgmath::{Matrix4, One};
use convert::format::{FnFmt, Mat};
use convert::image_namer::ImageNamer;
use db::{Database, ModelId};
use errors::Result;
use skeleton::{Skeleton, Transform, SMatrix};
use primitives::{self, Primitives};
use nitro::Model;
use petgraph::{Direction};
use petgraph::graph::NodeIndex;
use std::fmt::{self, Write};
use time;
use util::ins_set::InsOrderSet;
use connection::Connection;

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

pub fn write<W: Write>(
    w: &mut W,
    db: &Database,
    conn: &Connection,
    image_namer: &ImageNamer,
    model_id: ModelId,
) -> Result<()> {
    let model = &db.models[model_id];

    // We need invertible matrices since we're obliged to give values for
    // inverse bind matrices.
    use convert::make_invertible::make_invertible;
    let objects = &model.objects.iter()
        .map(|o| make_invertible(&o.matrix))
        .collect::<Vec<_>>();
    let prims = &Primitives::build(model, primitives::PolyType::TrisAndQuads, objects)?;
    let skel = &Skeleton::build(model, objects);

    let ctx = Ctx { model_id, model, db, conn, image_namer, objects, prims, skel };

    write_lines!(w,
        r##"<?xml version="1.0" encoding="utf-8"?>"##,
        r##"<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1">"##;
    )?;
    write_asset(w)?;
    write_library_images(w, &ctx)?;
    write_library_materials(w, &ctx)?;
    write_library_effects(w, &ctx)?;
    write_library_geometries(w, &ctx)?;
    write_library_controllers(w, &ctx)?;
    write_library_animations(w, &ctx)?;
    write_library_animation_clips(w, &ctx)?;
    write_library_visual_scenes(w, &ctx)?;
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

fn write_library_images<W: Write>(w: &mut W, ctx: &Ctx) -> Result<()> {
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

    write_lines!(w,
        r##"  <library_images>"##;
    )?;

    for name in image_names {
        write_lines!(w,
            r##"    <image id="image-{name}">"##,
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

fn write_library_materials<W: Write>(w: &mut W, ctx: &Ctx) -> Result<()> {
    write_lines!(w,
        r##"  <library_materials>"##;
    )?;
    for (i, mat) in ctx.model.materials.iter().enumerate() {
        write_lines!(w,
            r##"    <material id="material{i}" name="{name}">"##,
            r##"      <instance_effect url="#effect{i}"/>"##,
            r##"    </material>"##;
            i = i,
            name = mat.name.print_safe(),
        )?;
    }
    write_lines!(w,
        r##"  </library_materials>"##;
    )?;
    Ok(())
}

fn write_library_effects<W: Write>(w: &mut W, ctx: &Ctx) -> Result<()> {
    write_lines!(w,
        r##"  <library_effects>"##;
    )?;
    for (material_id, mat) in ctx.model.materials.iter().enumerate() {
        let mat_conn = &ctx.conn.models[ctx.model_id].materials[material_id];
        let image_name = match mat_conn.image_id() {
            Ok(Some(image_id)) => ctx.image_namer.names.get(&image_id),
            _ => None,
        };

        write_lines!(w,
            r##"    <effect id="effect{i}" name="{name}">"##,
            r##"      <profile_COMMON>"##;
            i = material_id, name = mat.name.print_safe(),
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
                r##"            <init_from>image-{image_name}</init_from>"##,
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

fn write_library_geometries<W: Write>(w: &mut W, ctx: &Ctx) -> Result<()> {
    let verts = &ctx.prims.vertices;

    write_lines!(w,
        r##"  <library_geometries>"##,
        r##"    <geometry id="geometry" name="{model_name}">"##,
        r##"      <mesh>"##;
        model_name = ctx.model.name.print_safe(),
    )?;

    write_lines!(w,
        r##"        <source id="positions">"##,
        r##"          <float_array id="positions-array" count="{num_floats}">{floats}</float_array>"##,
        r##"          <technique_common>"##,
        r##"            <accessor source="#positions-array" count="{num_verts}" stride="3">"##,
        r##"              <param name="X" type="float"/>"##,
        r##"              <param name="Y" type="float"/>"##,
        r##"              <param name="Z" type="float"/>"##,
        r##"            </accessor>"##,
        r##"          </technique_common>"##,
        r##"        </source>"##;
        num_floats = 3 * verts.len(), num_verts = verts.len(),
        floats = FnFmt(|f| {
            for v in verts {
                let pos = &v.position;
                write!(f, "{} {} {} ", pos[0], pos[1], pos[2])?;
            }
            Ok(())
        }),
    )?;

    write_lines!(w,
        r##"        <source id="texcoords">"##,
        r##"          <float_array id="texcoords-array" count="{num_floats}">{floats}</float_array>"##,
        r##"          <technique_common>"##,
        r##"            <accessor source="#texcoords-array" count="{num_verts}" stride="2">"##,
        r##"              <param name="S" type="float"/>"##,
        r##"              <param name="T" type="float"/>"##,
        r##"            </accessor>"##,
        r##"          </technique_common>"##,
        r##"        </source>"##;
        num_floats = 2 * verts.len(), num_verts = verts.len(),
        floats = FnFmt(|f| {
            for v in verts {
                let texcoord = &v.texcoord;
                write!(f, "{} {} ", texcoord[0], texcoord[1])?;
            }
            Ok(())
        }),
    )?;

    // Omit the colors if they are all white
    let omit_colors = verts.iter().all(|v| v.color == [1.0, 1.0, 1.0]);
    if !omit_colors {
        write_lines!(w,
            r##"        <source id="colors">"##,
            r##"          <float_array id="colors-array" count="{num_floats}">{floats}</float_array>"##,
            r##"          <technique_common>"##,
            r##"            <accessor source="#colors-array" count="{num_verts}" stride="3">"##,
            r##"              <param name="R" type="float"/>"##,
            r##"              <param name="G" type="float"/>"##,
            r##"              <param name="B" type="float"/>"##,
            r##"            </accessor>"##,
            r##"          </technique_common>"##,
            r##"        </source>"##;
            num_floats = 3 * verts.len(), num_verts = verts.len(),
            floats = FnFmt(|f| {
                for v in verts {
                    let color = &v.color;
                    write!(f, "{} {} {} ", color[0], color[1], color[2])?;
                }
                Ok(())
            }),
        )?;
    }

    write_lines!(w,
        r##"        <vertices id="vertices">"##,
        r##"          <input semantic="POSITION" source="#positions"/>"##,
        r##"          <input semantic="TEXCOORD" source="#texcoords"/>"##;
    )?;
    if !omit_colors {
        write_lines!(w,
            r##"          <input semantic="COLOR" source="#colors"/>"##;
        )?;
    }
    write_lines!(w,
        r#"        </vertices>"#;
    )?;

    for call in &ctx.prims.draw_calls {
        let indices = &ctx.prims.indices[call.index_range.clone()];
        write_lines!(w,
            r##"        <polylist material="material{mat_id}" count="{num_polys}">"##,
            r##"          <input semantic="VERTEX" source="#vertices" offset="0"/>"##,
            r##"          <vcount>{vcounts}</vcount>"##,
            r##"          <p>{indices}</p>"##,
            r##"        </polylist>"##;
            mat_id = call.mat_id, num_polys = indices.len() / 4,
            vcounts = FnFmt(|f| {
                let mut i = 0;
                while i < indices.len() {
                    if indices[i + 3] == 0xffff {
                        write!(f, "3 ")?;
                    } else {
                        write!(f, "4 ")?;
                    }
                    i += 4;
                }
                Ok(())
            }),
            indices = FnFmt(|f| {
                for &index in indices {
                    if index == 0xffff { continue; }
                    write!(f, "{} ", index)?;
                }
                Ok(())
            }),
        )?;

    }

    write_lines!(w,
        r##"      </mesh>"##,
        r##"    </geometry>"##,
        r##"  </library_geometries>"##;
    )?;

    Ok(())
}

fn write_library_controllers<W: Write>(w: &mut W, ctx: &Ctx) -> Result<()> {
    write_lines!(w,
        r##"  <library_controllers>"##;
    )?;

    let num_joints = ctx.skel.tree.node_count();

    write_lines!(w,
        r##"    <controller id="controller">"##,
        r##"      <skin source="#geometry">"##;
    )?;
    write_lines!(w,
        r##"        <source id="controller-joints">"##,
        r##"          <Name_array id="controller-joints-array" count="{num_joints}">{joints}</Name_array>"##,
        r##"          <technique_common>"##,
        r##"            <accessor source="#controller-joints-array" count="{num_joints}">"##,
        r##"              <param name="JOINT" type="Name"/>"##,
        r##"            </accessor>"##,
        r##"          </technique_common>"##,
        r##"        </source>"##;
        num_joints = num_joints,
        joints = FnFmt(|f| {
            for j in ctx.skel.tree.node_indices() {
                write!(f, "joint{} ", j.index())?;
            }
            Ok(())
        }),
    )?;

    write_lines!(w,
        r##"        <source id="controller-bind_poses">"##,
        r##"          <float_array id="controller-bind_poses-array" count="{num_floats}">{floats}</float_array>"##,
        r##"          <technique_common>"##,
        r##"            <accessor source="#controller-bind_poses-array" count="{num_joints}" stride="16">"##,
        r##"              <param name="TRANSFORM" type="float4x4"/>"##,
        r##"            </accessor>"##,
        r##"          </technique_common>"##,
        r##"        </source>"##;
        num_floats = 16 * num_joints, num_joints = num_joints,
        floats = FnFmt(|f| {
            for j in ctx.skel.tree.node_indices() {
                let inv_bind = &ctx.skel.tree[j].rest_world_to_local;
                write!(f, "{} ", Mat(inv_bind))?;
            }
            Ok(())
        }),
    )?;

    // We gives weights by first giving a list of all weights we're going to
    // use and then giving indices into the list with the vertices, so we start
    // by gathering all weights into a list. Since weights are floats, we can't
    // insert them into a HashMap directly, so we first encode them as a
    // fixed-point number. Remember to decode them when they come out!
    let mut all_weights = InsOrderSet::new();
    let encode = |x: f32| (x * 4096.0) as u32;
    let decode = |x: u32| x as f64 / 4096.0;
    all_weights.clear();
    for v in &ctx.skel.vertices {
        for influence in &v.influences {
            all_weights.insert(encode(influence.weight));
        }
    }
    write_lines!(w,
        r##"        <source id="controller-weights">"##,
        r##"          <float_array id="controller-weights-array" count="{num_weights}">{weights}</float_array>"##,
        r##"          <technique_common>"##,
        r##"            <accessor source="#controller-weights-array" count="{num_weights}">"##,
        r##"              <param name="WEIGHT" type="float"/>"##,
        r##"            </accessor>"##,
        r##"          </technique_common>"##,
        r##"        </source>"##;
        num_weights = all_weights.len(),
        weights = FnFmt(|f| {
            for &weight in all_weights.iter() {
                write!(f, "{} ", decode(weight))?;
            }
            Ok(())
        })
    )?;

    write_lines!(w,
        r##"        <joints>"##,
        r##"          <input semantic="JOINT" source="#controller-joints"/>"##,
        r##"          <input semantic="INV_BIND_MATRIX" source="#controller-bind_poses"/>"##,
        r##"        </joints>"##;
    )?;

    write_lines!(w,
        r##"        <vertex_weights count="{num_verts}">"##,
        r##"          <input semantic="JOINT" source="#controller-joints" offset="0"/>"##,
        r##"          <input semantic="WEIGHT" source="#controller-weights" offset="1"/>"##,
        r##"          <vcount>{vcount}</vcount>"##,
        r##"          <v>{v}</v>"##,
        r##"        </vertex_weights>"##;
        num_verts = ctx.skel.vertices.len(),
        vcount = FnFmt(|f| {
            for v in &ctx.skel.vertices {
                write!(f, "{} ", v.influences.len())?;
            }
            Ok(())
        }),
        v = FnFmt(|f| {
            for v in &ctx.skel.vertices {
                for influence in &v.influences {
                    write!(f, "{} {} ",
                        influence.joint.index(),
                        all_weights.get_index_from_value(&encode(influence.weight)).unwrap(),
                    )?;
                }
            }
            Ok(())
        }),
    )?;

    write_lines!(w,
        r##"      </skin>"##,
        r##"    </controller>"##,
        r##"  </library_controllers>"##;
    )?;

    Ok(())
}

fn write_library_animations<W: Write>(w: &mut W, ctx: &Ctx) -> Result<()> {
    let anims = &ctx.conn.models[ctx.model_id].animations;
    if anims.is_empty() { return Ok(()); }

    write_lines!(w,
        r##"  <library_animations>"##;
    )?;
    for &anim_id in anims {
        let anim = &ctx.db.animations[anim_id];
        let num_frames = anim.num_frames;

        for joint_id in ctx.skel.tree.node_indices() {
            let joint = &ctx.skel.tree[joint_id];
            let object_id = match joint.local_to_parent {
                Transform::SMatrix(SMatrix::Object { object_idx }) => object_idx,
                _ => continue,
            };

            write_lines!(w,
                r##"    <animation id="anim{anim_id}-joint{joint_id}">"##;
                anim_id = anim_id, joint_id = joint_id.index(),
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
                anim_id = anim_id, joint_id = joint_id.index(), num_frames = num_frames,
                times = FnFmt(|f| {
                    for frame in 0..num_frames {
                        write!(f, "{} ", frame as f64 * FRAME_LENGTH)?;
                    }
                    Ok(())
                }),
            )?;

            write_lines!(w,
                r##"      <source id="anim{anim_id}-joint{joint_id}-matrix">"##,
                r##"        <float_array id="anim{anim_id}-joint{joint_id}-matrix-array" count="{num_floats}">{mats}</float_array>"##,
                r##"        <technique_common>"##,
                r##"          <accessor source="#anim{anim_id}-joint{joint_id}-matrix-array" count="{num_frames}" stride="16">"##,
                r##"            <param name="TRANSFORM" type="float4x4"/>"##,
                r##"          </accessor>"##,
                r##"        </technique_common>"##,
                r##"      </source>"##;
                anim_id = anim_id, joint_id = joint_id.index(), num_floats = 16 * num_frames, num_frames = num_frames,
                mats = FnFmt(|f| {
                    for frame in 0..num_frames {
                        let mat = anim.objects_curves.get(object_id as usize)
                            .map(|trs| trs.sample_at(frame))
                            .unwrap_or_else(|| Matrix4::one());
                        write!(f, "{} ", Mat(&mat))?;
                    }
                    Ok(())
                }),
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
                anim_id = anim_id, joint_id = joint_id.index(), num_frames = num_frames,
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
                anim_id = anim_id, joint_id = joint_id.index(),
            )?;

            write_lines!(w,
                r##"      <channel source="#anim{anim_id}-joint{joint_id}-sampler" target="joint{joint_id}/transform"/>"##;
                anim_id = anim_id, joint_id = joint_id.index(),
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


fn write_library_animation_clips<W: Write>(w: &mut W, ctx: &Ctx) -> Result<()> {
    let anims = &ctx.conn.models[ctx.model_id].animations;
    if anims.is_empty() { return Ok(()); }

    write_lines!(w,
        r##"  <library_animation_clips>"##;
    )?;
    for &anim_id in anims {
        let anim = &ctx.db.animations[anim_id];
        check!(anim.num_frames != 0)?;
        let end_time = (anim.num_frames - 1) as f64 * FRAME_LENGTH;

        write_lines!(w,
            r##"    <animation_clip id="anim{anim_id}" name="{name}" end="{end_time}">"##;
            anim_id = anim_id, name = anim.name.print_safe(), end_time = end_time,
        )?;
        for joint_id in ctx.skel.tree.node_indices() {
            write_lines!(w,
                r##"      <instance_animation url="#anim{anim_id}-joint{joint_id}"/>"##;
                anim_id = anim_id, joint_id = joint_id.index(),
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

fn write_library_visual_scenes<W: Write>(w: &mut W, ctx: &Ctx) -> Result<()> {
    write_lines!(w,
        r#"  <library_visual_scenes>"#,
        r#"    <visual_scene id="scene0" name="{model_name}">"#;
        model_name = ctx.model.name.print_safe(),
    )?;

    write_joint_hierarchy(w, ctx)?;

    write_lines!(w,
        r##"      <node id="node" name="{model_name}" type="NODE">"##,
        r##"        <instance_controller url="#controller">"##,
        r##"          <skeleton>#joint{root_id}</skeleton>"##,
        r##"          <bind_material>"##,
        r##"            <technique_common>"##;
        model_name = ctx.model.name.print_safe(),
        root_id = ctx.skel.root.index(),
    )?;

    for i in 0..ctx.model.materials.len() {
        write_lines!(w,
            r##"              <instance_material symbol="material{i}" target="#material{i}">"##,
            r##"                <bind_vertex_input semantic="tc" input_semantic="TEXCOORD"/>"##,
            r##"              </instance_material>"##;
            i = i,
        )?;
    }

    write_lines!(w,
        r##"            </technique_common>"##,
        r##"          </bind_material>"##,
        r##"        </instance_controller>"##,
        r##"      </node>"##;
    )?;

    write_lines!(w,
        r##"    </visual_scene>"##,
        r##"  </library_visual_scenes>"##;
    )?;

    Ok(())
}

fn write_joint_hierarchy<W: Write>(w: &mut W, ctx: &Ctx) -> Result<()> {
    fn write_indent<W: Write>(w: &mut W, indent: u32) -> Result<()> {
        // Base indent
        write!(w, "      ")?;
        for _ in 0..indent {
            write!(w, "  ")?;
        }
        Ok(())
    }

    /// Write the name for a joint that will appear in DCC programs.
    fn write_joint_name<W: Write>(w: &mut W, ctx: &Ctx, node: NodeIndex) -> fmt::Result {
        match ctx.skel.tree[node].local_to_parent {
            Transform::Root =>
                write!(w, "__ROOT__"),
            Transform::SMatrix(SMatrix::Object { object_idx }) =>
                write!(w, "{}", ctx.model.objects[object_idx as usize].name.print_safe()),
            Transform::SMatrix(SMatrix::InvBind { inv_bind_idx }) =>
                write!(w, "__INV_BIND{}", inv_bind_idx),
            Transform::SMatrix(SMatrix::Uninitialized { stack_pos }) =>
                write!(w, "__UNINITIALIZED{}", stack_pos),
        }
    }

    fn write_rec<W: Write>(w: &mut W, ctx: &Ctx, node: NodeIndex, indent: u32) -> Result<()> {
        let tree = &ctx.skel.tree;

        write_indent(w, indent)?;
        write_lines!(w,
            r#"<node id="joint{joint_id}" sid="joint{joint_id}" name="{name}" type="JOINT">"#;
            joint_id = node.index(),
            name = FnFmt(|f| write_joint_name(f, ctx, node)),
        )?;

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
        write_indent(w, indent + 1)?;
        write_lines!(w, r#"<matrix sid="transform">{}</matrix>"#; Mat(&mat))?;

        let children = tree.neighbors_directed(node, Direction::Outgoing);
        for child in children {
            write_rec(w, ctx, child, indent + 1)?;
        }

        write_indent(w, indent)?;
        write!(w, "</node>\n")?;
        Ok(())
    }
    write_rec(w, ctx, ctx.skel.root, 0)
}

fn write_scene<W: Write>(w: &mut W) -> Result<()> {
    write_lines!(w,
        r##"  <scene>"##,
        r##"    <instance_visual_scene url="#scene0"/>"##,
        r##"  </scene>"##;
    )?;
    Ok(())
}
