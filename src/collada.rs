use errors::Result;
use geometry;
use geometry::GeometryData;
use gfx::GfxState;
use nitro::mdl::Model;
use render;
use std::collections::HashSet;
use std::fmt::Write;
use time;
use util::name;

struct Sink<'a, 'b: 'a> {
    geosink: geometry::Sink,
    model: &'a Model<'b>,
}

impl<'a, 'b: 'a> render::Sink for Sink<'a, 'b> {
    fn draw(&mut self, gs: &mut GfxState, mesh_id: u8, material_id: u8) -> Result<()> {
        let material = &self.model.materials[material_id as usize];
        gs.texture_mat = material.texture_mat;
        self.geosink.begin_mesh(material_id);
        self.geosink.cur_texture_dim = (material.width as u32, material.height as u32);
        gs.run_commands(&mut self.geosink, self.model.meshes[mesh_id as usize].commands)?;
        self.geosink.end_mesh();
        Ok(())
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

pub fn write<W: Write>(w: &mut W, model: &Model) -> Result<()> {
    let mut s = Sink {
        geosink: geometry::Sink::new(),
        model: model,
    };
    let mut r = render::Renderer::new();
    r.run_render_cmds(&mut s, &model.objects[..], model.render_cmds_cur)?;
    let geom = s.geosink.data();

    write!(w, cat!(
        r#"<?xml version="1.0" encoding="utf-8"?>"#,
        r#"<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1">"#,
    ))?;
    write_asset(w)?;
    write_library_images(w, model)?;
    write_library_materials(w, model)?;
    write_library_effects(w, model)?;
    write_library_geometries(w, model, &geom)?;
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

fn write_library_images<W: Write>(w: &mut W, model: &Model) -> Result<()> {
    let texture_names = model.materials.iter()
        .map(|mat| mat.name)
        .collect::<HashSet<_>>();

    write!(w, cat!(
        r#"  <library_images>"#,
    ))?;
    for name in texture_names {
        write!(w, cat!(
            r#"    <image id="{name}">"#,
            r#"      <init_from>{name}.png</init_from>"#,
            r#"    </image>"#,
            ),
            name = name::IdFmt(&name),
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

fn write_library_effects<W: Write>(w: &mut W, model: &Model) -> Result<()> {
    write!(w, cat!(
        r#"  <library_effects>"#,
    ))?;
    for (i, mat) in model.materials.iter().enumerate() {
        write!(w, cat!(
            r#"    <effect id="effect{i}" name="{name}">"#,
            r#"      <profile_COMMON>"#,
            ),
            i = i,
            name = name::IdFmt(&mat.name),
        )?;

        if let Some(texname) = mat.texture_name {
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
                r#"            <init_from>{texname}</init_from>"#,
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
                texname = name::IdFmt(&mat.name),
                wrap_s = wrap(mat.params.repeat_s(), mat.params.mirror_s()),
                wrap_t = wrap(mat.params.repeat_t(), mat.params.mirror_t()),
            )?;
        }

        write!(w, cat!(
            r#"        <technique sid="common">"#,
            r#"          <phong>"#,
            r#"            <diffuse>"#,
        ))?;
        if mat.texture_name.is_some() {
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
        if mat.texture_name.is_some() {
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
    for (i, mesh) in model.meshes.iter().enumerate() {
        let mesh_range = &geom.mesh_ranges[i];

        let num_vertices = (mesh_range.vertex_range.end - mesh_range.vertex_range.start) as usize;

        write!(w, cat!(
            r#"    <geometry id="geometry{i}" name="{name}">"#,
            r#"      <mesh>"#,
            ),
            i = i,
            name = name::IdFmt(&mesh.name),
        )?;

        write!(w, cat!(
            r#"        <source id="geometry{i}_positions">"#,
            ),
            i = i,
        )?;
        let num_floats = 3 * num_vertices;
        write!(w,
            r#"          <float_array id="geometry_{i}_positions_array" count="{num_floats}">"#,
            i = i,
            num_floats = num_floats,
        )?;
        for j in mesh_range.vertex_range.clone() {
            let pos = &geom.vertices[j as usize].position;
            write!(w, "{} {} {} ", pos[0], pos[1], pos[2])?;
        }
        write!(w, "</float_array>\n")?;
        write!(w, cat!(
            r#"          <technique_common>"#,
            r##"            <accessor source="#geometry{i}_positions_array" count="{num_verts}" stride="3">"##,
            r#"              <param name="X" type="float"/>"#,
            r#"              <param name="Y" type="float"/>"#,
            r#"              <param name="Z" type="float"/>"#,
            r#"            </accessor>"#,
            r#"          </technique_common>"#,
            r#"        </source>"#,
            ),
            i = i,
            num_verts = num_vertices,
        )?;

        write!(w, cat!(
            r#"        <source id="geometry{i}_texcoords">"#,
            ),
            i = i,
        )?;
        let num_floats = 2 * num_vertices;
        write!(w,
            r#"          <float_array id="geometry_{i}_texcoords_array" count="{num_floats}">"#,
            i = i,
            num_floats = num_floats,
        )?;
        for j in mesh_range.vertex_range.clone() {
            let texcoord = &geom.vertices[j as usize].texcoord;
            write!(w, "{} {} ", texcoord[0], texcoord[1])?;
        }
        write!(w, "</float_array>\n")?;
        write!(w, cat!(
            r#"          <technique_common>"#,
            r##"            <accessor source="#geometry{i}_texcoord_array" count="{num_verts}" stride="2">"##,
            r#"              <param name="S" type="float"/>"#,
            r#"              <param name="T" type="float"/>"#,
            r#"            </accessor>"#,
            r#"          </technique_common>"#,
            r#"        </source>"#,
            ),
            i = i,
            num_verts = num_vertices,
        )?;

        write!(w, cat!(
            r#"        <source id="geometry{i}_colors">"#,
            ),
            i = i,
        )?;
        let num_floats = 3 * num_vertices;
        write!(w,
            r#"          <float_array id="geometry_{i}_colors_array" count="{num_floats}">"#,
            i = i,
            num_floats = num_floats,
        )?;
        for j in mesh_range.vertex_range.clone() {
            let color = &geom.vertices[j as usize].color;
            write!(w, "{} {} {} ", color[0], color[1], color[2])?;
        }
        write!(w, "</float_array>\n")?;
        write!(w, cat!(
            r#"          <technique_common>"#,
            r##"            <accessor source="#geometry{i}_colors_array" count="{num_verts}" stride="3">"##,
            r#"              <param name="R" type="float"/>"#,
            r#"              <param name="G" type="float"/>"#,
            r#"              <param name="B" type="float"/>"#,
            r#"            </accessor>"#,
            r#"          </technique_common>"#,
            r#"        </source>"#,
            ),
            i = i,
            num_verts = num_vertices,
        )?;

        write!(w, cat!(
            r#"        <vertices id="geometry{i}_vertices">"#,
            r##"          <input semantic="POSITION" source="#geometry{i}_positions"/>"##,
            r##"          <input semantic="TEXCOORD" source="#geometry{i}_texcoords"/>"##,
            r##"          <input semantic="COLOR" source="#geometry{i}_colors"/>"##,
            r#"        </vertices>"#,
            ),
            i = i,
        )?;

        let num_tris = (mesh_range.index_range.end - mesh_range.index_range.start) / 3;
        write!(w, cat!(
            r#"        <triangles material="material{mat_id}" count="{num_tris}">"#,
            r##"          <input semantic="VERTEX" source="#geometry{i}_vertices" offset="0"/>"##,
            ),
            i = i,
            mat_id = mesh_range.mat_id,
            num_tris = num_tris,
        )?;
        write!(w,
            r#"          <p>"#,
        )?;
        let start_index = mesh_range.vertex_range.start;
        for j in mesh_range.index_range.clone() {
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

fn write_library_visual_scenes<W: Write>(w: &mut W, model: &Model, geom: &GeometryData) -> Result<()> {
    write!(w, cat!(
        r#"  <library_visual_scenes>"#,
        r#"    <visual_scene id="scene0" name="{model_name}">"#,
        ),
        model_name = name::IdFmt(&model.name),
    )?;
    for (i, mesh) in model.meshes.iter().enumerate() {
        let mesh_range = &geom.mesh_ranges[i];
        write!(w, cat!(
            r#"      <node id="node{i}" name="{mesh_name}" type="NODE">"#,
            r##"        <instance_geometry url="#geometry{i}">"##,
            r#"          <bind_material>"#,
            r#"            <technique_common>"#,
            r##"              <instance_material symbol="material{mat_id}" target="#material{mat_id}"/>"##,
            r#"            </technique_common>"#,
            r#"          </bind_material>"#,
            r#"        </instance_geometry>"#,
            r#"      </node>"#,
            ),
            i = i,
            mesh_name = name::IdFmt(&mesh.name),
            mat_id = mesh_range.mat_id,
        )?;
    }
    write!(w, cat!(
        r#"    </visual_scene>"#,
        r#"  </library_visual_scenes>"#,
    ))?;
    Ok(())
}

fn write_scene<W: Write>(w: &mut W) -> Result<()> {
    write!(w, cat!(
        r#"  <scene>"#,
        r##"    <instance_visual_scene url="#scene0"/>"##,
        r#"  </scene>"#,
    ))?;
    Ok(())
}
