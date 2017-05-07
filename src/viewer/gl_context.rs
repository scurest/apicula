use errors::Result;
use glium;

/// All the GL state that is independent of the model
/// being displayed.
pub struct GlContext {
    pub display: glium::Display,
    pub program: glium::Program,
    pub default_texture: glium::texture::Texture2d,
    pub error_texture: glium::texture::Texture2d,
}

impl GlContext {
    pub fn new(display: glium::Display) -> Result<GlContext> {
        let vertex_shader = include_str!("shaders/vert.glsl");
        let fragment_shader = include_str!("shaders/frag.glsl");
        let program_args =
            glium::program::ProgramCreationInput::SourceCode {
                vertex_shader,
                fragment_shader,
                geometry_shader: None,
                tessellation_control_shader: None,
                tessellation_evaluation_shader: None,
                transform_feedback_varyings: None,
                outputs_srgb: true,
                uses_point_size: false,
            };
        let program = glium::Program::new(&display, program_args).unwrap();

        // 1x1 white texture
        let default_image =
            glium::texture::RawImage2d::from_raw_rgba(
                vec![255, 255, 255, 255u8],
                (1,1),
            );
        let default_texture =
            glium::texture::Texture2d::new(&display, default_image).unwrap();

        // 1x1 magenta texture
        let error_image =
            glium::texture::RawImage2d::from_raw_rgba(
                vec![255, 0, 255, 255u8],
                (1,1),
            );
        let error_texture =
            glium::texture::Texture2d::new(&display, error_image).unwrap();

        Ok(GlContext {
            display,
            program,
            default_texture,
            error_texture,
        })
    }
}
