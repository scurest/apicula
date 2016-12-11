use cgmath::EuclideanSpace;
use cgmath::InnerSpace;
use cgmath::Matrix4;
use cgmath::PerspectiveFov;
use cgmath::Point3;
use cgmath::Rad;
use cgmath::Transform;
use cgmath::vec3;
use errors::Result;
use geometry;
use geometry::Vertex;
use gfx::GfxState;
use glium;
use nitro::mdl::Model;
use nitro::tex::image::gen_image;
use nitro::tex::Tex;
use render;
use std::f32::consts::PI;
use time;

implement_vertex!(Vertex, position, texcoord, color);

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

struct Eye {
    position: Point3<f32>,
    azimuth: f32,
    altitude: f32,
}

struct MouseState {
    pos: (i32, i32),
    grabbed: GrabState,
}

enum GrabState {
    NotGrabbed,
    Grabbed { saved_pos: (i32, i32) },
}

pub fn viewer(model: &Model, tex: &Tex) -> Result<()> {
    use glium::{DisplayBuild, Surface};
    let display = glium::glutin::WindowBuilder::new()
        .with_dimensions(512,384)
        .with_depth_buffer(24)
        .build_glium()
        .unwrap();

    let mut s = Sink {
        geosink: geometry::Sink::new(),
        model: model,
    };
    let mut r = render::Renderer::new();
    r.run_render_cmds(&mut s, &model.objects[..], model.render_cmds_cur)?;
    let geom = s.geosink.data;
    //println!("{:#?}", geom.vertices);
    let vertex_buffer = glium::VertexBuffer::new(
        &display,
        &geom.vertices
    ).unwrap();
    let indices = glium::IndexBuffer::new(
        &display,
        glium::index::PrimitiveType::TrianglesList,
        &geom.indices
    ).unwrap();

    // Build textures
    let textures = model.materials.iter()
        .map(|material| {
            let palinfo = material.palette_name
                .and_then(|name| tex.palinfo.iter().find(|info| info.name == name));
            let texinfo = material.texture_name
                .and_then(|name| tex.texinfo.iter().find(|info| info.name == name));
            texinfo.map(|texinfo| {
                let rgba = gen_image(tex, texinfo, palinfo).unwrap();
                let dim = (texinfo.params.width(), texinfo.params.height());
                let image = glium::texture::RawImage2d::from_raw_rgba_reversed(rgba, dim);
                glium::texture::Texture2d::new(&display, image).unwrap()
            })
        })
        .collect::<Vec<_>>();
    // The default image is just a 1x1 white texture. Or it should be. For some reason
    // I don't understand, on the Windows box I'm testing on, 1x1 textures don't seem
    // to work. The work-around is just to make it 2x1. :(
    let default_image = glium::texture::RawImage2d::from_raw_rgba(
        vec![255u8,255,255,255,255u8,255,255,255],
        (2,1)
    );
    let default_texture = glium::texture::Texture2d::new(&display, default_image).unwrap();

    let vertex_shader_src = r#"
        #version 140
        in vec3 position;
        in vec2 texcoord;
        in vec3 color;
        out vec2 v_texcoord;
        out vec3 v_color;
        uniform mat4 matrix;
        void main() {
            v_texcoord = texcoord;
            v_color = color;
            gl_Position = matrix * vec4(position, 1.0);
        }
    "#;

    let fragment_shader_src = r#"
        #version 140
        in vec2 v_texcoord;
        in vec3 v_color;
        out vec4 color;
        uniform sampler2D tex;
        void main() {
            vec4 sample = texture(tex, v_texcoord);
            if (sample.w == 0.0) discard;
            color = sample * vec4(v_color, 1.0);
        }
    "#;

    let program = glium::Program::new(&display,
        glium::program::ProgramCreationInput::SourceCode {
            vertex_shader: vertex_shader_src,
            fragment_shader: fragment_shader_src,
            geometry_shader: None,
            tessellation_control_shader: None,
            tessellation_evaluation_shader: None,
            transform_feedback_varyings: None,
            outputs_srgb: true,
            uses_point_size: false,
        }
    ).unwrap();

    let mut eye = Eye {
        position: Point3::new(0.0, 0.0, 0.0),
        azimuth: 0.0,
        altitude: 0.0,
    };

    let mut mouse = MouseState {
        pos: (0,0),
        grabbed: GrabState::NotGrabbed,
    };

    let mut t = vec3(0.0, 0.0, 0.0);
    let mut last_time = time::precise_time_s() as f32;
    loop {
        let (w, h) = display.get_window().unwrap()
            .get_inner_size_pixels().unwrap();
        let (w,h) = (w as i32, h as i32);
        let asp = w as f32 / h as f32;

        let mut target = display.draw();

        let middle_grey = (0.4666, 0.4666, 0.4666, 1.0);
        target.clear_color_srgb_and_depth(middle_grey, 1.0);

        let drawparams = glium::DrawParameters {
            depth: glium::Depth {
                test: glium::draw_parameters::DepthTest::IfLess,
                write: true,
                .. Default::default()
            },
            backface_culling: glium::draw_parameters::BackfaceCullingMode::CullClockwise,
            .. Default::default()
        };

        let persp = PerspectiveFov {
            fovy: Rad(1.1),
            aspect: asp,
            near: 0.01,
            far: 100.0,
        };
        let mat =
            Matrix4::from(persp) *
            Matrix4::from_angle_x(Rad(-eye.altitude)) *
            Matrix4::from_angle_y(Rad(-eye.azimuth)) *
            Matrix4::from_translation(-eye.position.to_vec());

        for range in geom.mesh_ranges.iter() {
            let tx = textures[range.mat_id as usize].as_ref()
                .unwrap_or(&default_texture);
            let mut sampler = glium::uniforms::Sampler::new(tx)
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
            let params = model.materials[range.mat_id as usize].params;
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
                &vertex_buffer,
                &indices.slice(range.index_range.clone()).unwrap(),
                &program,
                &uniforms,
                &drawparams,
            ).unwrap();
        }

        target.finish().unwrap();

        let xform =
            Matrix4::from_angle_y(Rad(eye.azimuth));

        let forward = xform.transform_vector(vec3(0.0, 0.0, -1.0));
        let side = xform.transform_vector(vec3(1.0, 0.0, 0.0));
        let up = xform.transform_vector(vec3(0.0, 1.0, 0.0));

        for ev in display.poll_events() {
            use glium::glutin::Event as Ev;
            use glium::glutin::ElementState as State;
            use glium::glutin::VirtualKeyCode as Key;
            match ev {
                Ev::Closed => return Ok(()),
                Ev::KeyboardInput(State::Pressed, _, Some(Key::S)) => t.x = -1.0,
                Ev::KeyboardInput(State::Pressed, _, Some(Key::W)) => t.x = 1.0,
                Ev::KeyboardInput(State::Pressed, _, Some(Key::A)) => t.y = -1.0,
                Ev::KeyboardInput(State::Pressed, _, Some(Key::D)) => t.y = 1.0,
                Ev::KeyboardInput(State::Pressed, _, Some(Key::F)) => t.z = -1.0,
                Ev::KeyboardInput(State::Pressed, _, Some(Key::R)) => t.z = 1.0,

                Ev::KeyboardInput(State::Released, _, Some(Key::S)) => t.x = 0.0,
                Ev::KeyboardInput(State::Released, _, Some(Key::W)) => t.x = 0.0,
                Ev::KeyboardInput(State::Released, _, Some(Key::A)) => t.y = 0.0,
                Ev::KeyboardInput(State::Released, _, Some(Key::D)) => t.y = 0.0,
                Ev::KeyboardInput(State::Released, _, Some(Key::F)) => t.z = 0.0,
                Ev::KeyboardInput(State::Released, _, Some(Key::R)) => t.z = 0.0,

                Ev::MouseInput(State::Pressed, glium::glutin::MouseButton::Left) => {
                    mouse.grabbed = GrabState::Grabbed { saved_pos: mouse.pos };
                    display.get_window().unwrap().set_cursor_position(w/2, h/2);
                    display.get_window().unwrap().set_cursor_state(glium::glutin::CursorState::Hide)?;
                }
                Ev::MouseInput(State::Released, glium::glutin::MouseButton::Left) => {
                    if let GrabState::Grabbed { saved_pos } = mouse.grabbed {
                        display.get_window().unwrap().set_cursor_state(glium::glutin::CursorState::Normal)?;
                        display.get_window().unwrap().set_cursor_position(saved_pos.0, saved_pos.1);
                    }
                    mouse.grabbed = GrabState::NotGrabbed;
                }
                Ev::Focused(false) => {
                    display.get_window().unwrap().set_cursor_state(glium::glutin::CursorState::Normal)?;
                    mouse.grabbed = GrabState::NotGrabbed;
                    t = vec3(0.0, 0.0, 0.0);
                }
                Ev::MouseMoved(x,y) => {
                    mouse.pos = (x,y);
                    if let GrabState::Grabbed { .. } = mouse.grabbed {
                        let (dx, dy) = (x - w/2, y - h/2);
                        eye.azimuth -= 0.01 * dx as f32;
                        eye.altitude -= 0.01 * dy as f32;
                        if eye.azimuth >= 2.0 * PI {
                            eye.azimuth -= 2.0*PI;
                        } else if eye.azimuth < 0.0 {
                            eye.azimuth += 2.0*PI;
                        }
                        if eye.altitude > 0.498*PI {
                            eye.altitude = 0.498*PI;
                        }
                        if eye.altitude < -0.498*PI {
                            eye.altitude = -0.498*PI;
                        }
                        display.get_window().unwrap().set_cursor_position(w/2, h/2);
                    }
                }
                _ => ()
            }
        }
        let cur_time = time::precise_time_s() as f32;
        let dt = cur_time - last_time;
        last_time = cur_time;

        let mag = t.magnitude();
        if mag != 0.0 {
            let tt = t / mag;
            eye.position += &(&forward * tt.x + &side * tt.y + &up * tt.z) * dt;
        }
    }
}
