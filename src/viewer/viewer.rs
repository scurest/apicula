use cgmath::EuclideanSpace;
use cgmath::InnerSpace;
use cgmath::Matrix4;
use cgmath::PerspectiveFov;
use cgmath::Point3;
use cgmath::Rad;
use cgmath::Transform;
use cgmath::vec3;
use cgmath::Vector3;
use errors::Result;
use files::FileHolder;
use geometry;
use geometry::GeometryData;
use geometry::Vertex;
use glium;
use nitro::mdl::Material;
use nitro::mdl::Model;
use nitro::tex::image::gen_image;
use nitro::tex::Tex;
use nitro::tex::TextureInfo;
use std::f32::consts::PI;
use time;
use util::name::Name;

implement_vertex!(Vertex, position, texcoord, color);

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

struct State<'a> {
    file_holder: FileHolder<'a>,
    cur_model: usize,
    eye: Eye,
    mouse: MouseState,
    move_dir: Vector3<f32>,
    last_time: f32,
    display: glium::Display,
    program: glium::Program,
    default_texture: glium::texture::Texture2d,
    model_data: ModelData,
}

/// Data needed to render a specific model.
struct ModelData {
    geo_data: GeometryData,
    vertex_buffer: glium::VertexBuffer<Vertex>,
    index_buffer: glium::IndexBuffer<u16>,
    textures: Vec<Option<glium::texture::Texture2d>>,
}

impl ModelData {
    fn new(display: &glium::Display, model: &Model, texs: &[Tex]) -> Result<ModelData> {
        let geo_data = geometry::build(model)?;
        let vertex_buffer = glium::VertexBuffer::new(
            display,
            &geo_data.vertices
        )?;
        let index_buffer = glium::IndexBuffer::new(
            display,
            glium::index::PrimitiveType::TrianglesList,
            &geo_data.indices
        )?;
        let textures = build_textures(display, &model.materials[..], texs);
        Ok(ModelData {
            geo_data: geo_data,
            vertex_buffer: vertex_buffer,
            index_buffer: index_buffer,
            textures: textures,
        })
    }
}

fn find_matching_texture_info<'a, 'b>(texs: &'b [Tex<'a>], texture_name: Name)
-> Result<(&'b Tex<'a>, &'b TextureInfo)> {
    for tex in texs {
        let res = tex.texinfo.iter().find(|info| texture_name == info.name);
        if let Some(texinfo) = res {
            return Ok((tex, texinfo))
        }
    }
    Err(format!("couldn't find texture named {}", texture_name).into())
}

fn build_textures(display: &glium::Display, materials: &[Material], texs: &[Tex])
-> Vec<Option<glium::texture::Texture2d>> {
    // Seriously horrible function follows :-(
    materials.iter()
        .map(|material| {
            let tex_texinfo = material.texture_name.map(|name| {
                find_matching_texture_info(texs, name)
            });
            let tex_texinfo_palinfo: Option<Result<_>> = tex_texinfo.map(|res| -> Result<_> {
                let (tex, texinfo) = res?;
                let palinfo = material.palette_name.and_then(|name| {
                    tex.palinfo.iter().find(|info| info.name == name)
                });
                Ok((tex, texinfo, palinfo))
            });
            let texture_res = tex_texinfo_palinfo.map(|res| -> Result<_> {
                let (tex, texinfo, palinfo) = res?;
                let rgba = gen_image(tex, texinfo, palinfo)?;
                let dim = (texinfo.params.width(), texinfo.params.height());
                let image = glium::texture::RawImage2d::from_raw_rgba_reversed(rgba, dim);
                Ok(glium::texture::Texture2d::new(display, image)?)
            });
            texture_res
        })
        .map(|res| {
            match res {
                Some(Ok(x)) => Some(x),
                Some(Err(e)) => {
                    error!("error generating texture: {:?}", e);
                    None
                }
                None => None,
            }
        })
        .collect()
}

pub fn viewer(file_holder: FileHolder) -> Result<()> {
    let num_models = file_holder.models.len();
    let num_animations = file_holder.animations.len();
    println!("Found {} models.", num_models);
    println!("Found {} animations.", num_animations);

    if num_models == 0 {
        println!("Nothing to do.");
        return Ok(())
    }

    let cur_model = 0;

    use glium::DisplayBuild;
    let display = glium::glutin::WindowBuilder::new()
        .with_dimensions(512,384)
        .with_depth_buffer(24)
        .build_glium()
        .unwrap();

    let model_data = ModelData::new(
        &display,
        &file_holder.models[cur_model],
        &file_holder.texs[..],
    )?;

    // The default image is just a 1x1 white texture. Or it should be. For some reason
    // I don't understand, on the Windows box I'm testing on, 1x1 textures don't seem
    // to work. The work-around is just to make it 2x1. :(
    let default_image = glium::texture::RawImage2d::from_raw_rgba(
        vec![255u8,255,255,255,255u8,255,255,255],
        (2,1),
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

    let eye = Eye {
        position: Point3::new(0.0, 0.0, 0.0),
        azimuth: 0.0,
        altitude: 0.0,
    };

    let mouse = MouseState {
        pos: (0,0),
        grabbed: GrabState::NotGrabbed,
    };

    let move_dir = vec3(0.0, 0.0, 0.0);

    let last_time = time::precise_time_s() as f32;

    let mut st = State {
        file_holder: file_holder,
        cur_model: cur_model,
        eye: eye,
        mouse: mouse,
        move_dir: move_dir,
        last_time: last_time,
        display: display,
        program: program,
        default_texture: default_texture,
        model_data: model_data,
    };

    run(&mut st)
}

fn run(st: &mut State) -> Result<()> {
    let drawparams = glium::DrawParameters {
        depth: glium::Depth {
            test: glium::draw_parameters::DepthTest::IfLess,
            write: true,
            .. Default::default()
        },
        backface_culling: glium::draw_parameters::BackfaceCullingMode::CullClockwise,
        .. Default::default()
    };

    loop {
        let (w, h) = st.display.get_window().unwrap()
            .get_inner_size_pixels().unwrap();
        let (w,h) = (w as i32, h as i32);
        let asp = w as f32 / h as f32;

        use glium::Surface;
        let mut target = st.display.draw();

        let middle_grey = (0.4666, 0.4666, 0.4666, 1.0);
        target.clear_color_srgb_and_depth(middle_grey, 1.0);

        let persp = PerspectiveFov {
            fovy: Rad(1.1),
            aspect: asp,
            near: 0.01,
            far: 100.0,
        };

        // Transform from world space into camera space
        let mat =
            Matrix4::from(persp) *
            Matrix4::from_angle_x(Rad(-st.eye.altitude)) *
            Matrix4::from_angle_y(Rad(-st.eye.azimuth)) *
            Matrix4::from_translation(-st.eye.position.to_vec());

        let model = &st.file_holder.models[st.cur_model];

        for call in st.model_data.geo_data.draw_calls.iter() {
            let texture = st.model_data.textures[call.mat_id as usize].as_ref()
                .unwrap_or(&st.default_texture);
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
                &st.model_data.vertex_buffer,
                &st.model_data.index_buffer.slice(call.index_range.clone()).unwrap(),
                &st.program,
                &uniforms,
                &drawparams,
            ).unwrap();
        }

        target.finish().unwrap();

        // Treating the eye as if it were inclined neither up nor down,
        // transform the forward/side/up basis in camera space into
        // world space. For knowing which direction to move when you
        // press WASD.
        let xform =
            Matrix4::from_angle_y(Rad(st.eye.azimuth));
        let forward = xform.transform_vector(vec3(0.0, 0.0, -1.0));
        let side = xform.transform_vector(vec3(1.0, 0.0, 0.0));
        let up = xform.transform_vector(vec3(0.0, 1.0, 0.0));

        for ev in st.display.poll_events() {
            use glium::glutin::Event as Ev;
            use glium::glutin::ElementState as Es;
            use glium::glutin::VirtualKeyCode as K;
            match ev {
                Ev::Closed => return Ok(()),
                Ev::KeyboardInput(Es::Pressed, _, Some(K::W)) => st.move_dir.x = 1.0,
                Ev::KeyboardInput(Es::Pressed, _, Some(K::S)) => st.move_dir.x = -1.0,
                Ev::KeyboardInput(Es::Pressed, _, Some(K::A)) => st.move_dir.y = -1.0,
                Ev::KeyboardInput(Es::Pressed, _, Some(K::D)) => st.move_dir.y = 1.0,
                Ev::KeyboardInput(Es::Pressed, _, Some(K::F)) => st.move_dir.z = -1.0,
                Ev::KeyboardInput(Es::Pressed, _, Some(K::R)) => st.move_dir.z = 1.0,

                Ev::KeyboardInput(Es::Released, _, Some(K::W)) => st.move_dir.x = 0.0,
                Ev::KeyboardInput(Es::Released, _, Some(K::S)) => st.move_dir.x = 0.0,
                Ev::KeyboardInput(Es::Released, _, Some(K::A)) => st.move_dir.y = 0.0,
                Ev::KeyboardInput(Es::Released, _, Some(K::D)) => st.move_dir.y = 0.0,
                Ev::KeyboardInput(Es::Released, _, Some(K::F)) => st.move_dir.z = 0.0,
                Ev::KeyboardInput(Es::Released, _, Some(K::R)) => st.move_dir.z = 0.0,

                Ev::MouseInput(Es::Pressed, glium::glutin::MouseButton::Left) => {
                    st.mouse.grabbed = GrabState::Grabbed { saved_pos: st.mouse.pos };
                    st.display.get_window().unwrap().set_cursor_position(w/2, h/2);
                    st.display.get_window().unwrap().set_cursor_state(glium::glutin::CursorState::Hide)?;
                }
                Ev::MouseInput(Es::Released, glium::glutin::MouseButton::Left) => {
                    if let GrabState::Grabbed { saved_pos } = st.mouse.grabbed {
                        st.display.get_window().unwrap().set_cursor_state(glium::glutin::CursorState::Normal)?;
                        st.display.get_window().unwrap().set_cursor_position(saved_pos.0, saved_pos.1);
                    }
                    st.mouse.grabbed = GrabState::NotGrabbed;
                }
                Ev::Focused(false) => {
                    st.display.get_window().unwrap().set_cursor_state(glium::glutin::CursorState::Normal)?;
                    st.mouse.grabbed = GrabState::NotGrabbed;
                    st.move_dir = vec3(0.0, 0.0, 0.0);
                }
                Ev::MouseMoved(x,y) => {
                    st.mouse.pos = (x,y);
                    if let GrabState::Grabbed { .. } = st.mouse.grabbed {
                        let (dx, dy) = (x - w/2, y - h/2);
                        st.eye.azimuth -= 0.01 * dx as f32;
                        st.eye.altitude -= 0.01 * dy as f32;
                        if st.eye.azimuth >= 2.0 * PI {
                            st.eye.azimuth -= 2.0*PI;
                        } else if st.eye.azimuth < 0.0 {
                            st.eye.azimuth += 2.0*PI;
                        }
                        if st.eye.altitude > 0.499*PI {
                            st.eye.altitude = 0.499*PI;
                        }
                        if st.eye.altitude < -0.499*PI {
                            st.eye.altitude = -0.499*PI;
                        }
                        st.display.get_window().unwrap().set_cursor_position(w/2, h/2);
                    }
                }
                Ev::KeyboardInput(Es::Pressed, _, Some(K::Period)) => {
                    if st.cur_model == st.file_holder.models.len() - 1 {
                        st.cur_model = 0;
                    } else {
                        st.cur_model += 1;
                    }
                    st.model_data = ModelData::new(
                        &st.display,
                        &st.file_holder.models[st.cur_model],
                        &st.file_holder.texs[..],
                    )?;
                }
                Ev::KeyboardInput(Es::Pressed, _, Some(K::Comma)) => {
                    if st.cur_model == 0 {
                        st.cur_model = st.file_holder.models.len() - 1;
                    } else {
                        st.cur_model -= 1;
                    }
                    st.model_data = ModelData::new(
                        &st.display,
                        &st.file_holder.models[st.cur_model],
                        &st.file_holder.texs[..],
                    )?;
                }
                _ => ()
            }
        }
        let cur_time = time::precise_time_s() as f32;
        let dt = cur_time - st.last_time;
        st.last_time = cur_time;

        let mag = st.move_dir.magnitude();
        if mag != 0.0 {
            let vel = st.move_dir / mag;
            st.eye.position += &(&forward * vel.x + &side * vel.y + &up * vel.z) * dt;
        }
    }
}
