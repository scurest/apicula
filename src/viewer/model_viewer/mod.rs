pub mod eye;
pub mod texture_cache;

pub use self::eye::Eye;

use cgmath::{PerspectiveFov, Rad, Matrix4};
use crate::db::Database;
use glium::{VertexBuffer, IndexBuffer, Frame, Surface, Program};
use crate::nitro::Model;
use crate::primitives::{Primitives, DrawCall, Vertex};
use self::texture_cache::{TextureCache, ImageId};
use super::{Z_NEAR, Z_FAR, FOV_Y};

type Display = glium::Display<glium::glutin::surface::WindowSurface>;

/// Model viewer.
///
/// Handles the GPU data needed to draw a model. Typical use is:
///
/// * use change_model to begin drawing a new model
/// * use update_vertices after, eg, you move to a new animation frame of the
///   same model
/// * use update_materials when the textures on the materials change
pub struct ModelViewer {
    pub eye: Eye,
    pub aspect_ratio: f32,
    pub light_on: bool,

    /// Program for unlit materials (using vertex colors)
    unlit_program: Program,
    /// Program for lit materials (using normals)
    lit_program: Program,

    vertex_buffer: Option<VertexBuffer<Vertex>>,
    index_buffer: Option<IndexBuffer<u16>>,
    draw_calls: Vec<DrawCall>,
    texture_cache: TextureCache,
    material_map: Vec<MaterialTextureBinding>,
}

/// Tells you what GL texture to use for a given material.
#[derive(Clone)]
pub enum MaterialTextureBinding {
    /// Used when the material had not texture.
    None,
    /// Used when the material had a texture but we couldn't resolve it.
    Missing,
    /// Use the given image.
    ImageId(ImageId),
}

impl ModelViewer {
    pub fn new(display: &Display) -> ModelViewer {
        let unlit_vertex_shader = include_str!("shaders/vert_unlit.glsl");
        let lit_vertex_shader = include_str!("shaders/vert_lit.glsl");
        let fragment_shader = include_str!("shaders/frag.glsl");
        let program_args =
            glium::program::ProgramCreationInput::SourceCode {
                vertex_shader: unlit_vertex_shader,
                fragment_shader,
                geometry_shader: None,
                tessellation_control_shader: None,
                tessellation_evaluation_shader: None,
                transform_feedback_varyings: None,
                outputs_srgb: true,
                uses_point_size: false,
            };
        let unlit_program = Program::new(display, program_args).unwrap();
        let program_args =
            glium::program::ProgramCreationInput::SourceCode {
                vertex_shader: lit_vertex_shader,
                fragment_shader,
                geometry_shader: None,
                tessellation_control_shader: None,
                tessellation_evaluation_shader: None,
                transform_feedback_varyings: None,
                outputs_srgb: true,
                uses_point_size: false,
            };
        let lit_program = Program::new(display, program_args).unwrap();

        ModelViewer {
            eye: Default::default(),
            aspect_ratio: 1.0,
            light_on: true,
            unlit_program,
            lit_program,
            vertex_buffer: None,
            index_buffer: None,
            texture_cache: TextureCache::new(display),
            draw_calls: vec![],
            material_map: vec![],
        }
    }

    /// Changes the viewed model.
    pub fn change_model(
        &mut self,
        display: &Display,
        db: &Database,
        prim: Primitives,
        material_map: Vec<MaterialTextureBinding>,
    ) {
        use glium::index::PrimitiveType;

        let vb = VertexBuffer::dynamic(display, &prim.vertices).unwrap();
        let ib = IndexBuffer::new(
            display,
            PrimitiveType::TrianglesList,
            &prim.indices).unwrap();
        self.vertex_buffer = Some(vb);
        self.index_buffer = Some(ib);

        self.draw_calls = prim.draw_calls;

        // Simple cache size management: clear everything when we change models
        self.texture_cache.clear();

        self.material_map = material_map;
        self.populate_texture_cache(display, db);
    }

    /// Update the vertices of the model (eg. when the position change because
    /// it is being animated). Cannot change the number of vertices,
    /// connectivity, etc.
    pub fn update_vertices(&mut self, vertices: &[Vertex]) {
        self.vertex_buffer.as_mut().unwrap().write(vertices);
    }

    /// Updates the list of material-image bindings.
    pub fn update_materials(
        &mut self,
        display: &Display,
        db: &Database,
        material_map: Vec<MaterialTextureBinding>,
    ) {
        self.material_map = material_map;

        self.populate_texture_cache(display, db)
    }

    /// Ensures all the images used by the material_map are in the texture
    /// cache.
    fn populate_texture_cache(&mut self, display: &Display, db: &Database) {
        for binding in &self.material_map {
            if let MaterialTextureBinding::ImageId(ref image_id) = binding {
                self.texture_cache.create(display, db, image_id.clone());
            }
        }
    }

    pub fn draw(&self, target: &mut Frame, model: &Model) {
        // Do nothing if there isn't vertex/index data
        let vertex_buffer = match self.vertex_buffer {
            Some(ref vb) => vb,
            None => return,
        };
        let index_buffer = match self.index_buffer {
            Some(ref ib) => ib,
            None => return,
        };

        // Model-view-projection matrix
        let model_view = self.eye.model_view();
        let persp: Matrix4<f32> = PerspectiveFov {
            fovy: Rad(FOV_Y),
            aspect: self.aspect_ratio,
            near: Z_NEAR,
            far: Z_FAR,
        }.into();
        let model_view_persp = persp * model_view;
        let model_view_persp: [[f32; 4]; 4] = model_view_persp.into();

        // Do each draw call
        for call in &self.draw_calls {
            let material = &model.materials[call.mat_id as usize];

            let texture = match self.material_map.get(call.mat_id as usize) {
                Some(&MaterialTextureBinding::None) =>
                    self.texture_cache.white_texture(),
                Some(&MaterialTextureBinding::Missing) =>
                    self.texture_cache.error_texture(),
                Some(&MaterialTextureBinding::ImageId(ref image_id)) =>
                    self.texture_cache.lookup(image_id.clone()),
                None => self.texture_cache.error_texture(),
            };
            let sampler = {
                use glium::uniforms::*;
                let mut s = Sampler::new(texture);

                s.1.minify_filter = MinifySamplerFilter::Nearest;
                s.1.magnify_filter = MagnifySamplerFilter::Nearest;

                // Texture wrapping/mirroring/clamping
                let wrap_fn = |repeat, mirror| {
                    match (repeat, mirror) {
                        (false, _) => SamplerWrapFunction::Clamp,
                        (true, false) => SamplerWrapFunction::Repeat,
                        (true, true) => SamplerWrapFunction::Mirror,
                    }
                };
                let params = &material.params;
                s.1.wrap_function.0 = wrap_fn(params.repeat_s(), params.mirror_s());
                s.1.wrap_function.1 = wrap_fn(params.repeat_t(), params.mirror_t());

                s
            };

            let indices = &index_buffer.slice(call.index_range.clone()).unwrap();

            let draw_params = glium::DrawParameters {
                depth: glium::Depth {
                    test: glium::draw_parameters::DepthTest::IfLess,
                    write: true,
                    .. Default::default()
                },
                backface_culling: {
                    use glium::draw_parameters::BackfaceCullingMode as Mode;
                    match (material.cull_backface, material.cull_frontface) {
                        (false, false) => Mode::CullingDisabled,
                        (true, false) => Mode::CullClockwise,
                        (false, true) => Mode::CullCounterClockwise,
                        (true, true) => continue,
                    }
                },
                blend: glium::Blend::alpha_blending(),
                .. Default::default()
            };

            if !call.used_normals || !self.light_on {
                let uniforms = uniform! {
                    matrix: model_view_persp,
                    alpha: material.alpha,
                    tex: sampler,
                };
                target.draw(
                    vertex_buffer,
                    indices,
                    &self.unlit_program,
                    &uniforms,
                    &draw_params,
                ).unwrap();
            } else {
                let uniforms = uniform! {
                    matrix: model_view_persp,
                    light_vec: [0.0, -0.624695, -0.78086877f32],
                    light_color: [1.0, 1.0, 1.0f32],
                    diffuse_color: material.diffuse,
                    ambient_color: material.ambient,
                    emission_color: material.emission,
                    alpha: material.alpha,
                    tex: sampler,
                };
                target.draw(
                    vertex_buffer,
                    indices,
                    &self.lit_program,
                    &uniforms,
                    &draw_params,
                ).unwrap();
            }
        }
    }
}
