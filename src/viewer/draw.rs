use cgmath::Matrix4;
use errors::Result;
use primitives::{Primitives, Vertex};
use glium::{self, VertexBuffer, IndexBuffer,
    Display, Frame, DrawParameters, Surface,
};
use glium::texture::Texture2d;
use nitro::model::Material;
use viewer::gl_context::GlContext;
use viewer::state::ViewState;
use db::Database;


/// GL data needed to render the scene.
pub struct DrawingData {
    obj_geom_data: Result<GLPrimitives>,
    /// textures[i] contains the GL texture that needs to be
    /// bound for materials[i]; either `Ok(None)` (use default
    /// texture), `Ok(Some(tex))` (use tex), or `Err(e)` (use
    /// error texture).
    textures: Vec<Result<Option<Texture2d>>>,
    /// Cache of the `ViewState` that was used to generate the
    /// other members. Caching itle ts us update less data when
    /// we need to change this (eg. we don't need to rebuild the
    /// textures if the model we're viewing didn't change).
    view_state: ViewState,
}

/// Primitive data that's been uploaded to the GPU for drawing.
struct GLPrimitives {
    primitives: Primitives,
    vertex_buffer: VertexBuffer<Vertex>,
    index_buffer: IndexBuffer<u16>,
}

impl DrawingData {
    pub fn from_view_state(
        display: &Display,
        db: &Database,
        view_state: &ViewState,
    ) -> DrawingData {
        let model = &db.models[view_state.model_id];

        let obj_geom_data =
            GLPrimitives::from_view_state(display, db, view_state);

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
        display: &Display,
        db: &Database,
        new_view_state: &ViewState,
    ) {
        let model_changed = self.view_state.model_id != new_view_state.model_id;
        if model_changed {
            // Regen everything from scratch
            *self = DrawingData::from_view_state(display, db, new_view_state);
            return;
        }

        let anim_changed = self.view_state.anim_state != new_view_state.anim_state;
        if anim_changed {
            // We can reuse the textures.
            self.obj_geom_data =
                GLPrimitives::from_view_state(display, db, new_view_state);
            self.view_state = new_view_state.clone();
            return;
        }

        // Only the eye changed. There are actually render commands
        // that depend on this (for billboard-type stuff) but they're
        // not implemented, so we can reuse the geometry. Do nothing.
        self.view_state = new_view_state.clone();
    }


    pub fn draw(
        &self,
        db: &Database,
        ctx: &GlContext,
        target: &mut Frame,
        draw_params: &DrawParameters
    ) {
        if let Ok(ref obj_geom_data) = self.obj_geom_data {
            let model = &db.models[self.view_state.model_id];

            let mvp = self.view_state.eye.model_view_persp();

            for call in &obj_geom_data.primitives.draw_calls {
                let texture =
                    match self.textures[call.mat_id as usize] {
                        Ok(Some(ref tex)) => tex,
                        Ok(None) => &ctx.default_texture,
                        Err(_) => &ctx.error_texture,
                    };

                let sampler = {
                    use glium::uniforms::*;

                    let mut s = Sampler::new(texture);

                    s.1.minify_filter = MinifySamplerFilter::Nearest;
                    s.1.magnify_filter = MagnifySamplerFilter::Nearest;

                    // Set the correct wrap function (mirror, repeat, clamp)
                    let wrap_fn = |repeat, mirror| {
                        match (repeat, mirror) {
                            (false, _) => SamplerWrapFunction::Clamp,
                            (true, false) => SamplerWrapFunction::Repeat,
                            (true, true) => SamplerWrapFunction::Mirror,
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

impl GLPrimitives {
    fn from_view_state(
        display: &glium::Display,
        db: &Database,
        view_state: &ViewState,
    ) -> Result<GLPrimitives>
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

        let primitives = Primitives::build(model, &objects[..])?;

        let vertex_buffer =
            glium::VertexBuffer::new(display, &primitives.vertices)?;

        use glium::index::PrimitiveType;
        let index_buffer =
            IndexBuffer::new(display, PrimitiveType::TrianglesList, &primitives.indices)?;

        Ok(GLPrimitives {
            primitives,
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
        let image = RawImage2d::from_raw_rgba_reversed(&rgba, dim);
        Ok(Some(Texture2d::new(display, image)?))
    };

    materials.iter().enumerate()
        .map(|(id, material)| {
            let res = build_texture(material);
            if let Err(ref e) = res {
                error!("error generating texture for material {}: {}", id, e);
            }
            res
        })
        .collect()
}
