use cgmath::Matrix4;
use errors::Result;
use files::FileHolder;
use geometry::build_without_joints as build_geometry;
use geometry::GeometryDataWithoutJoints as GeometryData;
use geometry::Vertex;
use glium;
use glium::Frame;
use glium::Surface;
use nitro::jnt::object::to_matrix as bca_object_to_matrix;
use nitro::mdl::Material;
use nitro::mdl::Model;
use nitro::tex::image::gen_image;
use nitro::tex::Tex;
use nitro::tex::texpal::find_tex;
use nitro::tex::texpal::TexPalPair;
use viewer::eye::Eye;

pub struct State<'a> {
    pub file_holder: FileHolder<'a>,
    pub eye: Eye,
    pub model_data: ModelData,
    pub display: glium::Display,
    pub program: glium::Program,
    pub default_texture: glium::texture::Texture2d,
}

pub struct ModelData {
    index: usize,
    geom: GeometryData,
    vertex_buffer: glium::VertexBuffer<Vertex>,
    index_buffer: glium::IndexBuffer<u16>,
    textures: Vec<Option<glium::texture::Texture2d>>,
    objects: Vec<Matrix4<f64>>,
    anim_data: Option<AnimData>,
}

#[derive(Copy, Clone)]
struct AnimData {
    index: usize,
    cur_frame: u16,
}

impl<'a> State<'a> {
    pub fn model(&self) -> &Model {
        &self.file_holder.models[self.model_data.index]
    }

    pub fn draw(&self, target: &mut Frame, draw_params: &glium::DrawParameters) -> Result<()> {
        let mat = self.eye.model_view_persp();

        for call in self.model_data.geom.draw_calls.iter() {
            let model = self.model();

            let texture = self.model_data.textures[call.mat_id as usize].as_ref()
                .unwrap_or(&self.default_texture);
            let mut sampler = glium::uniforms::Sampler::new(texture)
                .magnify_filter(glium::uniforms::MagnifySamplerFilter::Nearest)
                .minify_filter(glium::uniforms::MinifySamplerFilter::Nearest);
            let wrap_fn = |repeat, mirror| {
                use glium::uniforms::SamplerWrapFunction as Wrap;
                match (repeat, mirror) {
                    (false, _) => Wrap::Clamp,
                    (true, false) => Wrap::Repeat,
                    (true, true) => Wrap::Mirror,
                }
            };
            let params = model.materials[call.mat_id as usize].params;
            sampler.1.wrap_function.0 = wrap_fn(params.repeat_s(), params.mirror_s());
            sampler.1.wrap_function.1 = wrap_fn(params.repeat_t(), params.mirror_t());

            let uniforms = uniform! {
                matrix: [
                    [mat.x.x, mat.x.y, mat.x.z, mat.x.w],
                    [mat.y.x, mat.y.y, mat.y.z, mat.y.w],
                    [mat.z.x, mat.z.y, mat.z.z, mat.z.w],
                    [mat.w.x, mat.w.y, mat.w.z, mat.w.w],
                ],
                tex: sampler,
            };

            target.draw(
                &self.model_data.vertex_buffer,
                &self.model_data.index_buffer.slice(call.index_range.clone()).unwrap(),
                &self.program,
                &uniforms,
                &draw_params,
            ).unwrap();
        }

        Ok(())
    }
}

impl ModelData {
    pub fn new(file_holder: &FileHolder, display: &glium::Display, index: usize) -> Result<ModelData> {
        let model = &file_holder.models[index];
        let objects = model.objects.iter().map(|o| o.xform).collect::<Vec<_>>();
        let geom = build_geometry(model, &objects[..])?;
        let vertex_buffer = glium::VertexBuffer::new(
            display,
            &geom.vertices
        )?;
        let index_buffer = glium::IndexBuffer::new(
            display,
            glium::index::PrimitiveType::TrianglesList,
            &geom.indices
        )?;
        let textures = build_textures(
            display,
            &model.materials[..],
            &file_holder.texs[..]
        );
        let anim_data = None;
        Ok(ModelData {
            index: index,
            geom: geom,
            vertex_buffer: vertex_buffer,
            index_buffer: index_buffer,
            textures: textures,
            objects: objects,
            anim_data: anim_data,
        })
    }

    pub fn has_animation(&self) -> bool {
        self.anim_data.is_some()
    }

    pub fn prev_model(&mut self, file_holder: &FileHolder, display: &glium::Display) -> Result<()> {
        let num_models = file_holder.models.len();
        if num_models == 1 { return Ok(()); }

        let index = (self.index - 1 + num_models) % num_models;
        *self = ModelData::new(file_holder, display, index)?;

        Ok(())
    }
    pub fn next_model(&mut self, file_holder: &FileHolder, display: &glium::Display) -> Result<()> {
        let num_models = file_holder.models.len();
        if num_models == 1 { return Ok(()); }

        let index = (self.index + 1) % num_models;
        *self = ModelData::new(file_holder, display, index)?;

        Ok(())
    }

    pub fn prev_anim(&mut self, file_holder: &FileHolder) -> Result<()> {
        let num_animations = file_holder.animations.len();
        if num_animations == 0 { return Ok(()); }

        // The index of the animation, or None for the rest pose.
        let mut anim_id = self.anim_data.as_ref().map(|a| a.index);

        let max_anim_id = file_holder.animations.len() - 1;
        let num_objects = file_holder.models[self.index].objects.len();
        loop {
            anim_id = match anim_id {
                None => Some(max_anim_id),
                Some(i) if i == 0 => None,
                Some(i) => Some(i - 1),
            };
            // Skip over any animations with the wrong number of objects
            if let Some(i) = anim_id {
                if file_holder.animations[i].objects.len() != num_objects {
                    continue;
                }
            }
            break;
        }

        self.set_anim_data(file_holder, anim_id.map(|id|
            AnimData { index: id, cur_frame: 0 }
        ))?;

        Ok(())
    }
    pub fn next_anim(&mut self, file_holder: &FileHolder) -> Result<()> {
        if file_holder.animations.is_empty() { return Ok(()); }

        let mut anim_id = self.anim_data.as_ref().map(|a| a.index);

        let max_anim_id = file_holder.animations.len() - 1;
        let num_objects = file_holder.models[self.index].objects.len();
        loop {
            anim_id = match anim_id {
                None => Some(0),
                Some(i) if i == max_anim_id => None,
                Some(i) => Some(i + 1),
            };
            if let Some(i) = anim_id {
                if file_holder.animations[i].objects.len() != num_objects {
                    continue;
                }
            }
            break;
        }

        self.set_anim_data(file_holder, anim_id.map(|id|
            AnimData { index: id, cur_frame: 0 }
        ))?;

        Ok(())
    }

    pub fn next_anim_frame(&mut self, file_holder: &FileHolder) -> Result<()> {
        let mut anim_data = self.anim_data
            .expect("next frame called on unanimated model");
        let anim = &file_holder.animations[anim_data.index];

        let next_frame = anim_data.cur_frame + 1;
        let next_frame = if next_frame == anim.num_frames { 0 } else { next_frame };

        anim_data.cur_frame = next_frame;

        self.set_anim_data(file_holder, Some(anim_data))
    }

    fn set_anim_data(&mut self, file_holder: &FileHolder, anim_data: Option<AnimData>) -> Result<()> {
        let model = &file_holder.models[self.index];

        if let Some(anim_data) = anim_data {
            let anim = &file_holder.animations[anim_data.index];
            let it = self.objects.iter_mut()
                .zip(anim.objects.iter());
            for (obj, anim_obj) in it {
                *obj = bca_object_to_matrix(anim_obj, anim, anim_data.cur_frame)?;
            }
        } else {
            let it = self.objects.iter_mut()
                .zip(model.objects.iter());
            for (obj, model_obj) in it {
                *obj = model_obj.xform;
            }
        }

        self.geom = build_geometry(model, &self.objects[..])?;
        self.vertex_buffer.write(&self.geom.vertices);
        self.anim_data = anim_data;

        Ok(())
    }
}

fn build_textures(display: &glium::Display, materials: &[Material], texs: &[Tex])
-> Vec<Option<glium::texture::Texture2d>> {
    materials.iter()
        .map(|material| -> Result<Option<_>> {
            let pair = match TexPalPair::from_material(material) {
                Some(pair) => pair,
                None => return Ok(None), // no texture
            };
            let (tex, texinfo, palinfo) = find_tex(&texs[..], pair)
                .ok_or_else(|| format!("couldn't find texture named {}", pair.0))?;

            let rgba = gen_image(tex, texinfo, palinfo)?;
            let dim = (texinfo.params.width(), texinfo.params.height());
            let image = glium::texture::RawImage2d::from_raw_rgba_reversed(rgba, dim);
            Ok(Some(glium::texture::Texture2d::new(display, image)?))
        })
        .map(|res| {
            res.unwrap_or_else(|e| {
                error!("error generating texture: {:?}", e);
                None
            })
        })
        .collect()
}
