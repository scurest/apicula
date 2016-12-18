use cgmath::Matrix4;
use convert::image_names::ImageNames;
use convert::image_names::TexturePalettePair;
use errors::Result;
use geometry;
use geometry::GeometryData;
use geometry::Vertex;
use joint_builder::Kind;
use joint_builder::Weight;
use nitro::mdl::Model;
use petgraph::Direction;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use std::fmt;
use std::fmt::Write;
use time;
use util::name;

struct Mat<'a>(&'a Matrix4<f64>);

impl<'a> fmt::Display for Mat<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
            self.0.x.x, self.0.y.x, self.0.z.x, self.0.w.x,
            self.0.x.y, self.0.y.y, self.0.z.y, self.0.w.y,
            self.0.x.z, self.0.y.z, self.0.z.z, self.0.w.z,
            self.0.x.w, self.0.y.w, self.0.z.w, self.0.w.w,
        )
    }
}

/// Concatenate strings with a new line interposed after each.
macro_rules! cat {
    () => { "\n" };
    (,) => { "\n" };
    ($e:expr) => { concat!($e, "\n") };
    ($e:expr,) => { concat!($e, "\n") };
    ($e:expr, $($es:expr),*) => { concat!($e, "\n", cat!($($es),*)) };
    ($($es:expr),*,) => { cat!($($es),*) }
}

pub fn write<W: Write>(w: &mut W, model: &Model, image_names: &ImageNames) -> Result<()> {
    let geom = geometry::build(model)?;

    write!(w, cat!(
        r#"<?xml version="1.0" encoding="utf-8"?>"#,
        r#"<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1">"#,
    ))?;
    write_asset(w)?;
    write_library_images(w, image_names)?;
    write_library_materials(w, model)?;
    write_library_effects(w, model, image_names)?;
    write_library_geometries(w, model, &geom)?;
    write_library_controllers(w, &geom)?;
    write_library_visual_scenes(w, model, &geom)?;
    write_scene(w)?;
    write!(w, cat!(
        r#"</COLLADA>"#,
    ))?;
    Ok(())
}

fn write_asset<W: Write>(w: &mut W) -> Result<()> {
    let now = time::now_utc();
    let iso8601_datetime = time::strftime("%FT%TZ", &now)?;
    write!(w, cat!(
        r#"  <asset>"#,
        r#"    <created>{time}</created>"#,
        r#"    <modified>{time}</modified>"#,
        r#"  </asset>"#,
        ),
        time = iso8601_datetime,
    )?;
    Ok(())
}

fn write_library_images<W: Write>(w: &mut W, image_names: &ImageNames) -> Result<()> {
    write!(w, cat!(
        r#"  <library_images>"#,
    ))?;
    for name in image_names.values() {
        write!(w, cat!(
            r#"    <image id="{name}">"#,
            r#"      <init_from>{name}.png</init_from>"#,
            r#"    </image>"#,
            ),
            name = name,
        )?;
    }
    write!(w, cat!(
        r#"  </library_images>"#,
    ))?;
    Ok(())
}

fn write_library_materials<W: Write>(w: &mut W, model: &Model) -> Result<()> {
    write!(w, cat!(
        r#"  <library_materials>"#,
    ))?;
    for (i, mat) in model.materials.iter().enumerate() {
        write!(w, cat!(
            r#"    <material id="material{i}" name="{name}">"#,
            r##"      <instance_effect url="#effect{i}"/>"##,
            r#"    </material>"#,
            ),
            i = i,
            name = name::IdFmt(&mat.name),
        )?;
    }
    write!(w, cat!(
        r#"  </library_materials>"#,
    ))?;
    Ok(())
}

fn write_library_effects<W: Write>(w: &mut W, model: &Model, image_names: &ImageNames) -> Result<()> {
    write!(w, cat!(
        r#"  <library_effects>"#,
    ))?;
    for (i, mat) in model.materials.iter().enumerate() {
        let image_name = mat.texture_name
            .map(|texname| TexturePalettePair {
                texture_name: texname,
                palette_name: mat.palette_name,
            })
            .and_then(|pair| image_names.get(&pair));

        write!(w, cat!(
            r#"    <effect id="effect{i}" name="{name}">"#,
            r#"      <profile_COMMON>"#,
            ),
            i = i,
            name = name::IdFmt(&mat.name),
        )?;

        if let Some(name) = image_name {
            let wrap = |repeat, mirror| {
                match (repeat, mirror) {
                    (false, _) => "CLAMP",
                    (true, false) => "WRAP",
                    (true, true) => "MIRROR",
                }
            };
            write!(w, cat!(
                r#"        <newparam sid="Image-surface">"#,
                r#"          <surface type="2D">"#,
                r#"            <init_from>{image_name}</init_from>"#,
                r#"            <format>A8R8G8B8</format>"#,
                r#"          </surface>"#,
                r#"        </newparam>"#,
                r#"        <newparam sid="Image-sampler">"#,
                r#"          <sampler2D>"#,
                r#"            <source>Image-surface</source>"#,
                r#"            <wrap_s>{wrap_s}</wrap_s>"#,
                r#"            <wrap_t>{wrap_t}</wrap_t>"#,
                r#"            <minfilter>NEAREST</minfilter>"#,
                r#"            <magfilter>NEAREST</magfilter>"#,
                r#"            <mipfilter>NEAREST</mipfilter>"#,
                r#"          </sampler2D>"#,
                r#"        </newparam>"#,
                ),
                image_name = name,
                wrap_s = wrap(mat.params.repeat_s(), mat.params.mirror_s()),
                wrap_t = wrap(mat.params.repeat_t(), mat.params.mirror_t()),
            )?;
        }

        write!(w, cat!(
            r#"        <technique sid="common">"#,
            r#"          <phong>"#,
            r#"            <diffuse>"#,
        ))?;
        if image_name.is_some() {
            write!(w, cat!(
                r#"              <texture texture="Image-sampler" texcoord=""/>"#,
            ))?;
        } else {
            write!(w, cat!(
                r#"              <color>1 1 1 1</color>"#,
            ))?;
        }
        write!(w, cat!(
            r#"            </diffuse>"#,
            r#"            <transparent opaque="A_ONE">"#,
        ))?;
        if image_name.is_some() {
            write!(w, cat!(
                r#"              <texture texture="Image-sampler" texcoord=""/>"#,
            ))?;
        } else {
            write!(w, cat!(
                r#"              <color>0 0 0 1</color>"#,
            ))?;
        }
        write!(w, cat!(
            r#"            </transparent>"#,
            r#"          </phong>"#,
            r#"        </technique>"#,
            r#"      </profile_COMMON>"#,
            r#"    </effect>"#,
        ))?;
    }
    write!(w, cat!(
        r#"  </library_effects>"#,
    ))?;
    Ok(())
}

fn write_library_geometries<W: Write>(w: &mut W, model: &Model, geom: &GeometryData) -> Result<()> {
    write!(w, cat!(
        r#"  <library_geometries>"#,
    ))?;
    for (i, call) in geom.draw_calls.iter().enumerate() {
        let mesh = &model.meshes[call.mesh_id as usize];

        let num_vertices = (call.vertex_range.end - call.vertex_range.start) as usize;

        write!(w, cat!(
            r#"    <geometry id="geometry{i}" name="{name}">"#,
            r#"      <mesh>"#,
            ),
            i = i,
            name = name::IdFmt(&mesh.name),
        )?;

        fn write_float_source<W: Write, F>(
            w: &mut W,
            i: usize,
            name: &'static str,
            params: &[&'static str],
            vertices: &[Vertex],
            f: F
        ) -> Result<()>
            where F: Fn(&Vertex) -> &[f32]
        {
            let num_verts = vertices.len();
            let num_params = params.len();
            let num_floats = num_params * num_verts;

            write!(w, cat!(
                r#"        <source id="geometry{i}_{name}">"#,
                ),
                i = i,
                name = name
            )?;
            write!(w,
                r#"          <float_array id="geometry_{i}_{name}_array" count="{num_floats}">"#,
                i = i,
                name = name,
                num_floats = num_floats,
            )?;
            for v in vertices {
                for float in f(v) {
                    write!(w, "{} ", float)?;
                }
            }
            write!(w, "</float_array>\n")?;
            write!(w, cat!(
                r#"          <technique_common>"#,
                r##"            <accessor source="#geometry{i}_{name}_array" count="{num_verts}" stride="{num_params}">"##,
                ),
                i = i,
                name = name,
                num_verts = num_verts,
                num_params = num_params,
            )?;
            for param_name in params {
                write!(w, cat!(
                    r#"              <param name="{param_name}" type="float"/>"#,
                    ),
                    param_name = param_name,
                )?;
            }
            write!(w, cat!(
                r#"            </accessor>"#,
                r#"          </technique_common>"#,
                r#"        </source>"#,
            ))?;
            Ok(())
        }
        let vert_range = call.vertex_range.start as usize .. call.vertex_range.end as usize;
        let verts = &geom.vertices[vert_range];
        write_float_source(
            w,
            i,
            "positions",
            &["X", "Y", "Z"],
            verts,
            |v| { &v.position }
        )?;
        write_float_source(
            w,
            i,
            "texcoords",
            &["S", "T"],
            verts,
            |v| { &v.texcoord }
        )?;

        // Omit the colors if they are all white
        let omit_colors = verts.iter().all(|v| v.color == [1.0, 1.0, 1.0]);
        if !omit_colors {
            write_float_source(
                w,
                i,
                "colors",
                &["R", "G", "B"],
                verts,
                |v| { &v.color }
            )?;
        }

        write!(w, cat!(
            r#"        <vertices id="geometry{i}_vertices">"#,
            r##"          <input semantic="POSITION" source="#geometry{i}_positions"/>"##,
            r##"          <input semantic="TEXCOORD" source="#geometry{i}_texcoords"/>"##,
            ),
            i = i,
        )?;
        if !omit_colors {
            write!(w, cat!(
                r##"          <input semantic="COLOR" source="#geometry{i}_colors"/>"##,
                ),
                i = i,
            )?;
        }
        write!(w, cat!(
            r#"        </vertices>"#,
        ))?;

        let num_tris = (call.index_range.end - call.index_range.start) / 3;
        write!(w, cat!(
            r#"        <triangles material="material{mat_id}" count="{num_tris}">"#,
            r##"          <input semantic="VERTEX" source="#geometry{i}_vertices" offset="0"/>"##,
            ),
            i = i,
            mat_id = call.mat_id,
            num_tris = num_tris,
        )?;
        write!(w,
            r#"          <p>"#,
        )?;
        let start_index = call.vertex_range.start;
        for j in call.index_range.clone() {
            let index = &geom.indices[j] - start_index;
            write!(w, "{} ", index)?;
        }
        write!(w, "</p>\n")?;
        write!(w, cat!(
            r#"        </triangles>"#,
        ))?;

        write!(w, cat!(
            r#"      </mesh>"#,
            r#"    </geometry>"#,
        ))?;
    }
    write!(w, cat!(
        r#"  </library_geometries>"#,
    ))?;
    Ok(())
}

fn write_library_controllers<W: Write>(w: &mut W, geom: &GeometryData) -> Result<()> {
    write!(w, cat!(
        r#"  <library_controllers>"#,
    ))?;
    for (i, call) in geom.draw_calls.iter().enumerate() {
        write!(w, cat!(
            r#"    <controller id="controller{i}">"#,
            r##"      <skin source="#geometry{i}">"##,
            ),
            i = i,
        )?;
        write!(w, cat!(
            r#"        <source id="controller{i}-joints">"#,
            ),
            i = i,
        )?;
        let count = geom.joint_data.tree.node_count();
        write!(w,
            r#"          <IDREF_array id="controller{i}-joints-array" count="{count}">"#,
            i = i,
            count = count,
        )?;
        for j in 0..count {
            write!(w, "joint{} ", j)?;
        }
        write!(w, "</IDREF_array>\n")?;
        write!(w, cat!(
            r#"          <technique_common>"#,
            r##"            <accessor source="#controller{i}-joints-array" count="{count}" stride="1">"##,
            r#"              <param name="JOINT" type="Name"/>"#,
            r#"            </accessor>"#,
            r#"          </technique_common>"#,
            r#"        </source>"#,
            ),
            i = i,
            count = count
        )?;

        write!(w, cat!(
            r#"        <source id="controller{i}-bind_poses">"#,
            ),
            i = i,
        )?;
        write!(w,
            r#"          <float_array id="controller{i}-bind_poses-array" count="{num_floats}">"#,
            i = i,
            num_floats = 16 * count,
        )?;
        for j in 0..count {
            let inv_bind = geom.joint_data.tree[NodeIndex::new(j)].inv_bind_matrix;
            write!(w, "{} ", Mat(&inv_bind))?;
        }
        write!(w, "</float_array>\n")?;
        write!(w, cat!(
            r#"          <technique_common>"#,
            r##"            <accessor source="#controller{i}-joints-array" count="{count}" stride="16">"##,
            r#"              <param name="TRANSFORM" type="float4x4"/>"#,
            r#"            </accessor>"#,
            r#"          </technique_common>"#,
            r#"        </source>"#,
            ),
            i = i,
            count = count
        )?;

        write!(w, cat!(
            r#"        <source id="controller{i}-weights">"#,
            r#"          <float_array id="controller-weights-array" count="1">1</float_array>"#,
            r#"          <technique_common>"#,
            r##"            <accessor source="#controller-joints-array" count="1" stride="1">"##,
            r#"              <param name="WEIGHT" type="float"/>"#,
            r#"            </accessor>"#,
            r#"          </technique_common>"#,
            r#"        </source>"#,
            ),
            i = i,
        )?;
        write!(w, cat!(
            r#"        <joints>"#,
            r##"          <input semantic="JOINT" source="#controller{i}-joints"/>"##,
            r##"          <input semantic="INV_BIND_MATRIX" source="#controller{i}-bind_poses"/>"##,
            r#"        </joints>"#,
            ),
            i = i,
        )?;
        let num_verts = call.vertex_range.end - call.vertex_range.start;
        write!(w, cat!(
            r#"        <vertex_weights count="{num_verts}">"#,
            r##"          <input semantic="JOINT" source="#controller{i}-joints" offset="0"/>"##,
            r##"          <input semantic="WEIGHT" source="#controller{i}-weights" offset="1"/>"##,
            ),
            i = i,
            num_verts = num_verts,
        )?;
        write!(w, r#"          <vcount>"#)?;
        for _ in 0..num_verts {
            write!(w, "1 ")?;
        }
        write!(w, "</vcount>\n")?;
        write!(w, r#"          <v>"#)?;
        for j in 0..num_verts {
            let vert_index = call.vertex_range.start + j;
            let joint_index = geom.joint_data.vertices[vert_index as usize].index();
            write!(w, "{} 0 ", joint_index)?;
        }
        write!(w, "</v>\n")?;

        write!(w, cat!(
            r#"        </vertex_weights>"#,
            r#"      </skin>"#,
            r#"    </controller>"#,
        ))?;
    }
    write!(w, cat!(
        r#"  </library_controllers>"#,
    ))?;

    Ok(())
}

fn write_library_visual_scenes<W: Write>(w: &mut W, model: &Model, geom: &GeometryData) -> Result<()> {
    write!(w, cat!(
        r#"  <library_visual_scenes>"#,
        r#"    <visual_scene id="scene0" name="{model_name}">"#,
        ),
        model_name = name::IdFmt(&model.name),
    )?;

    write_joint_heirarchy(w, model, geom)?;

    for (i, call) in geom.draw_calls.iter().enumerate() {
        let mesh = &model.meshes[call.mesh_id as usize];
        write!(w, cat!(
            r#"      <node id="node{i}" name="{mesh_name}" type="NODE">"#,
            r##"        <instance_controller url="#controller{i}">"##,
            r#"          <bind_material>"#,
            r#"            <technique_common>"#,
            r##"              <instance_material symbol="material{mat_id}" target="#material{mat_id}"/>"##,
            r#"            </technique_common>"#,
            r#"          </bind_material>"#,
            r#"        </instance_controller>"#,
            r#"      </node>"#,
            ),
            i = i,
            mesh_name = name::IdFmt(&mesh.name),
            mat_id = call.mat_id,
        )?;
    }
    write!(w, cat!(
        r#"    </visual_scene>"#,
        r#"  </library_visual_scenes>"#,
    ))?;
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
    fn write<W: Write>(w: &mut W, model: &Model, tree: &Graph<Weight, ()>, node: NodeIndex, indent: u32) -> Result<()> {
        write_indent(w, indent)?;
        write!(w, r#"<node id="joint{}" "#, node.index())?;
        match tree[node].kind {
            Kind::Root => (),
            Kind::Object(id) => {
                let object = &model.objects[id as usize];
                write!(w, r#"name="{}" "#, name::IdFmt(&object.name))?;
            }
            Kind::UndefinedStackSlot(pos) => {
                write!(w, r#"name="__STACK{}__" "#, pos)?;
            }
        }
        write!(w, "type=\"JOINT\">\n")?;

        match tree[node].kind {
            Kind::Object(id) => {
                let mat = model.objects[id as usize].xform;
                write_indent(w, indent + 1)?;
                write!(w, "<matrix>{}</matrix>\n", Mat(&mat))?;
            }
            _ => (),
        }

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
    write!(w, cat!(
        r#"  <scene>"#,
        r##"    <instance_visual_scene url="#scene0"/>"##,
        r#"  </scene>"#,
    ))?;
    Ok(())
}
