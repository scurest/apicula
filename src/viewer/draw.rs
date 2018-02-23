use cgmath::Matrix4;
use errors::Result;
use geometry::build_without_joints as build_geometry;
use geometry::GeometryDataWithoutJoints as GeometryData;
use geometry::Vertex;
use glium;
use glium::Frame;
use glium::Surface;
use glium::texture::Texture2d;
use nitro::model::Material;
use viewer::gl_context::GlContext;
use viewer::state::ViewState;
use db::Database;

implement_vertex!(Vertex, position, texcoord, color);

/// GL data needed to render the scene.
pub struct DrawingData {
    obj_geom_data: Result<ObjectGeometryData>,
    /// textures[i] contains the GL texture that needs to be
    /// bound for materials[i]; either `Ok(None)` (use default
    /// texture), `Ok(Some(tex))` (use tex), or `Err(e)` (use
    /// error texture).
    textures: Vec<Result<Option<glium::texture::Texture2d>>>,
    /// Cache of the `ViewState` that was used to generate the
    /// other members. Caching it lets us update less data when
    /// we need to change this (eg. we don't need to rebuild the
    /// textures if the model we're viewing didn't change).
    view_state: ViewState,
}

struct ObjectGeometryData {
    objects: Vec<Matrix4<f64>>,
    geom: GeometryData,
    vertex_buffer: glium::VertexBuffer<Vertex>,
    index_buffer: glium::IndexBuffer<u16>,
}

impl DrawingData {
    pub fn from_view_state(
        display: &glium::Display,
        db: &Database,
        view_state: &ViewState,
    ) -> DrawingData {
        let model = &db.models[view_state.model_id];

        let obj_geom_data =
            ObjectGeometryData::from_view_state(display, db, view_state);

        let textures =
            build_textures(display, db, &model.materials[..]);

        DrawingData {
            obj_geom_data,
            textures,
            view_state: view_state.clone(),
        }
    }

    pub fn has_error(&self) -> bool {
        self.obj_geom_data.is_err()
    }

    /// Update the `DrawingData` when the `ViewState` changes. This requires
    /// less work than rebuilding from scratch.
    pub fn change_view_state(
        &mut self,
        display: &glium::Display,
        db: &Database,
        view_state: &ViewState,
    ) {
        if view_state.model_id == self.view_state.model_id {
            if view_state.anim_state == self.view_state.anim_state {
                // Only the eye changed. There are actually render commands
                // that depend on this (for billboard-type stuff) but they're
                // not implemented, so we can reuse the geometry. Do nothing.
                self.view_state = view_state.clone();
                return;
            } else {
                // Animation changed.
                // We should try reusing the old `ObjectGeometryData`'s buffers,
                // but this is kind of a pain because we need to do something
                // like `replace_with`. Since the biggest gain is from not
                // rebuilding textures, this is TODO for now.
                self.obj_geom_data =
                    ObjectGeometryData::from_view_state(display, db, view_state);
                self.view_state = view_state.clone();
            }
        } else {
            // Model changed, regen everything from scratch
            *self = DrawingData::from_view_state(display, db, view_state);
        }
    }


    pub fn draw(
        &self,
        db: &Database,
        ctx: &GlContext,
        target: &mut Frame,
        draw_params: &glium::DrawParameters
    ) {
        if let Ok(ref obj_geom_data) = self.obj_geom_data {
            let model = &db.models[self.view_state.model_id];

            let mvp = self.view_state.eye.model_view_persp();

            for call in &obj_geom_data.geom.draw_calls {
                let texture =
                    match self.textures[call.mat_id as usize] {
                        Ok(Some(ref tex)) => tex,
                        Ok(None) => &ctx.default_texture,
                        Err(_) => &ctx.error_texture,
                    };

                let sampler = {
                    use glium::uniforms as uni;

                    let mut s = uni::Sampler::new(texture);

                    s.1.minify_filter = uni::MinifySamplerFilter::Nearest;
                    s.1.magnify_filter = uni::MagnifySamplerFilter::Nearest;

                    // Set the correct wrap function (mirror, repeat, clamp)
                    let wrap_fn = |repeat, mirror| {
                        match (repeat, mirror) {
                            (false, _) => uni::SamplerWrapFunction::Clamp,
                            (true, false) => uni::SamplerWrapFunction::Repeat,
                            (true, true) => uni::SamplerWrapFunction::Mirror,
                        }
                    };
                    let params = &model.materials[call.mat_id as usize].params;
                    s.1.wrap_function.0 = wrap_fn(params.repeat_s, params.mirror_s);
                    s.1.wrap_function.1 = wrap_fn(params.repeat_t, params.mirror_t);

                    s
                };

                let uniforms = uniform! {
                    matrix: [
                        [mvp.x.x, mvp.x.y, mvp.x.z, mvp.x.w],
                        [mvp.y.x, mvp.y.y, mvp.y.z, mvp.y.w],
                        [mvp.z.x, mvp.z.y, mvp.z.z, mvp.z.w],
                        [mvp.w.x, mvp.w.y, mvp.w.z, mvp.w.w],
                    ],
                    tex: sampler,
                };

                let indices =
                    &obj_geom_data.index_buffer.slice(call.index_range.clone()).unwrap();

                target.draw(
                    &obj_geom_data.vertex_buffer,
                    indices,
                    &ctx.program,
                    &uniforms,
                    draw_params,
                ).unwrap();
            }
        }
    }
}

impl ObjectGeometryData {
    fn from_view_state(
        display: &glium::Display,
        db: &Database,
        view_state: &ViewState,
    ) -> Result<ObjectGeometryData>
    {
        let model = &db.models[view_state.model_id];

        let objects: Vec<Matrix4<f64>> =
            if let Some(ref anim_state) = view_state.anim_state {
                let anim = &db.animations[anim_state.anim_id];
                anim.objects_curves.iter()
                    .map(|trs_curves| trs_curves.sample_at(anim_state.cur_frame))
                    .collect()
            } else {
                model.objects.iter()
                    .map(|o| o.xform)
                    .collect()
            };

        let geom = build_geometry(model, &objects[..])?;

        let vertex_buffer =
            glium::VertexBuffer::new(display, &geom.vertices)?;

        let index_buffer = glium::IndexBuffer::new(
            display,
            glium::index::PrimitiveType::TrianglesList,
            &geom.indices
        )?;

        Ok(ObjectGeometryData {
            objects,
            geom,
            vertex_buffer,
            index_buffer,
        })
    }
}


fn build_textures(display: &glium::Display, db: &Database, materials: &[Material])
-> Vec<Result<Option<Texture2d>>> {
    use nitro::decode_image::decode;
    use glium::texture::RawImage2d;

    let build_texture = |material: &Material| -> Result<Option<Texture2d>> {
        if material.texture_name.is_none() {
            return Ok(None);
        }
        let texture_name = material.texture_name.unwrap();

        let texture_id = match db.textures_by_name.get(&texture_name) {
            Some(&x) => x,
            None => { bail!("no texture named {}", texture_name); }
        };
        let texture = &db.textures[texture_id];

        let palette = material.palette_name
            .and_then(|name| db.palettes_by_name.get(&name))
            .map(|&id| &db.palettes[id]);

        let rgba = decode(texture, palette)?;

        let dim = (texture.params.width, texture.params.height);
        let image = RawImage2d::from_raw_rgba_reversed(rgba, dim);
        Ok(Some(Texture2d::new(display, image)?))
    };

    materials.iter().enumerate()
        .map(|(id, material)| {
            let res = build_texture(material);
            if let Err(ref e) = res {
                error!("error generating texture for material {}: {:?}", id, e);
            }
            res
        })
        .collect()
}
