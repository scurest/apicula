use errors::Result;
use glium;

/// All the GL state that is independent of the model
/// being displayed.
pub struct GlContext {
    pub display: glium::Display,
    pub program: glium::Program,
    pub default_texture: glium::texture::Texture2d,
}

impl GlContext {
    pub fn new(display: glium::Display) -> Result<GlContext> {
        let vertex_shader_src = include_str!("shaders/vert.glsl");
        let fragment_shader_src = include_str!("shaders/frag.glsl");
        let program_args =
            glium::program::ProgramCreationInput::SourceCode {
                vertex_shader: vertex_shader_src,
                fragment_shader: fragment_shader_src,
                geometry_shader: None,
                tessellation_control_shader: None,
                tessellation_evaluation_shader: None,
                transform_feedback_varyings: None,
                outputs_srgb: true,
                uses_point_size: false,
            };
        let program = glium::Program::new(&display, program_args).unwrap();

        // The default image is just a 1x1 white texture. Or it should be. For some reason
        // I don't understand, on the Windows box I'm testing on, 1x1 textures don't seem
        // to work. The work-around is just to make it 2x1. :(
        let default_image =
            glium::texture::RawImage2d::from_raw_rgba(
                vec![255u8,255,255,255,255u8,255,255,255],
                (2,1),
            );
        let default_texture =
            glium::texture::Texture2d::new(&display, default_image).unwrap();

        Ok(GlContext {
            display: display,
            program: program,
            default_texture: default_texture,
        })
    }
}
