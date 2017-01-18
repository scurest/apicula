use cgmath::EuclideanSpace;
use cgmath::InnerSpace;
use cgmath::Matrix4;
use cgmath::PerspectiveFov;
use cgmath::Point3;
use cgmath::Rad;
use cgmath::Transform;
use cgmath::vec2;
use cgmath::vec3;
use cgmath::Vector2;
use cgmath::Vector3;
use errors::Result;
use files::FileHolder;
use geometry::build_without_joints as build_geometry;
use geometry::GeometryDataWithoutJoints as GeometryData;
use geometry::Vertex;
use glium;
use glium::backend::glutin_backend::WinRef;
use glium::Surface;
use nitro::jnt::object::to_matrix as bca_object_to_matrix;
use nitro::mdl::Material;
use nitro::mdl::Model;
use nitro::tex::image::gen_image;
use nitro::tex::Tex;
use nitro::tex::texpal::find_tex;
use nitro::tex::texpal::TexPalPair;
use std::f32::consts::PI;
use time;

implement_vertex!(Vertex, position, texcoord, color);

struct Eye {
    pub position: Point3<f32>,
    pub azimuth: f32,
    pub altitude: f32,
    pub aspect_ratio: f32,
}

impl Eye {
    pub fn model_view(&self) -> Matrix4<f32> {
        let mv =
            Matrix4::from_angle_x(Rad(-self.altitude)) *
            Matrix4::from_angle_y(Rad(-self.azimuth)) *
            Matrix4::from_translation(-self.position.to_vec());
        mv
    }
    pub fn model_view_persp(&self) -> Matrix4<f32> {
        let persp = PerspectiveFov {
            fovy: Rad(1.1),
            aspect: self.aspect_ratio,
            near: 0.01,
            far: 100.0,
        };
        Matrix4::from(persp) * self.model_view()
    }
    pub fn move_by(&mut self, dv: Vector3<f32>) {
        // Treating the eye as if it were inclined neither up nor down,
        // transform the forward/side/up basis in camera space into
        // world space.
        let t = Matrix4::from_angle_y(Rad(self.azimuth));
        let forward = t.transform_vector(vec3(0.0, 0.0, -1.0));
        let side = t.transform_vector(vec3(1.0, 0.0, 0.0));
        let up = t.transform_vector(vec3(0.0, 1.0, 0.0));

        self.position += forward * dv.x + side * dv.y + up * dv.z;
    }
    pub fn free_look(&mut self, dv: Vector2<f32>) {
        self.azimuth -= dv.x;
        self.altitude -= dv.y;

        // Wrap once (expect dv to be small) for azimuth
        if self.azimuth >= 2.0 * PI {
            self.azimuth -= 2.0 * PI;
        } else if self.azimuth < 0.0 {
            self.azimuth += 2.0 * PI;
        }

        // Clamp into allowable altitude range to avoid singularities
        // at the poles.
        let max_alt = 0.499 * PI;
        let min_alt = -max_alt;
        self.altitude =
            if self.altitude < min_alt { min_alt }
            else if self.altitude > max_alt { max_alt }
            else { self.altitude };
    }
}

struct MouseState {
    pos: (i32, i32),
    grabbed: GrabState,
}

enum GrabState {
    NotGrabbed,
    Grabbed { saved_pos: (i32, i32) },
}

impl MouseState {
    // Performs "best-effort" setting of the cursor position. If
    // setting the position fails, no error is signalled, but
    // self.pos is also not updated.
    fn set_position(&mut self, window: &WinRef, (x, y): (i32, i32)) {
        let res = window.set_cursor_position(x, y);
        match res {
            Ok(()) => self.pos = (x,y),
            Err(_) => (),
        }
    }
}

struct State<'a> {
    file_holder: FileHolder<'a>,
    eye: Eye,
    model_data: ModelData,
    display: glium::Display,
    program: glium::Program,
    default_texture: glium::texture::Texture2d,
}

/// Data needed to render a specific model.
struct ModelData {
    index: usize,
    geo_data: GeometryData,
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
}

impl ModelData {
    pub fn new(file_holder: &FileHolder, display: &glium::Display, index: usize) -> Result<ModelData> {
        let model = &file_holder.models[index];
        let objects = model.objects.iter().map(|o| o.xform).collect::<Vec<_>>();
        let geo_data = build_geometry(model, &objects[..])?;
        let vertex_buffer = glium::VertexBuffer::new(
            display,
            &geo_data.vertices
        )?;
        let index_buffer = glium::IndexBuffer::new(
            display,
            glium::index::PrimitiveType::TrianglesList,
            &geo_data.indices
        )?;
        let textures = build_textures(
            display,
            &model.materials[..],
            &file_holder.texs[..]
        );
        let anim_data = None;
        Ok(ModelData {
            index: index,
            geo_data: geo_data,
            vertex_buffer: vertex_buffer,
            index_buffer: index_buffer,
            textures: textures,
            objects: objects,
            anim_data: anim_data,
        })
    }
    pub fn prev_model(&mut self, file_holder: &FileHolder, display: &glium::Display) -> Result<()> {
        let id = self.index;
        let max_id = file_holder.models.len() - 1;
        let next_id = if id == 0 { max_id } else { id - 1 };
        *self = ModelData::new(file_holder, display, next_id)?;
        Ok(())
    }
    pub fn next_model(&mut self, file_holder: &FileHolder, display: &glium::Display) -> Result<()> {
        let id = self.index;
        let max_id = file_holder.models.len() - 1;
        let next_id = if id == max_id { 0 } else { id + 1 };
        *self = ModelData::new(file_holder, display, next_id)?;
        Ok(())
    }
    pub fn prev_anim(&mut self, file_holder: &FileHolder) -> Result<()> {
        if file_holder.animations.is_empty() { return Ok(()); }

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

        self.set_anim_data(file_holder, anim_id.map(|id| AnimData {
            index: id,
            cur_frame: 0,
        }))?;

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
            // Skip over any animations with the wrong number of objects
            if let Some(i) = anim_id {
                if file_holder.animations[i].objects.len() != num_objects {
                    continue;
                }
            }
            break;
        }

        self.set_anim_data(file_holder, anim_id.map(|id| AnimData {
            index: id,
            cur_frame: 0,
        }))?;

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
    pub fn set_anim_data(&mut self, file_holder: &FileHolder, anim_data: Option<AnimData>) -> Result<()> {
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

        self.geo_data = build_geometry(model, &self.objects[..])?;
        self.vertex_buffer.write(&self.geo_data.vertices);
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

pub fn viewer(file_holder: FileHolder) -> Result<()> {
    let num_models = file_holder.models.len();
    let num_animations = file_holder.animations.len();
    println!("Found {} models.", num_models);
    println!("Found {} animations.", num_animations);

    if num_models == 0 {
        println!("Nothing to do.");
        return Ok(())
    }

    let initial_win_dim = (512, 384);

    use glium::DisplayBuild;
    let display = glium::glutin::WindowBuilder::new()
        .with_dimensions(initial_win_dim.0, initial_win_dim.1)
        .with_depth_buffer(24)
        .build_glium()
        .unwrap();

    let model_data = ModelData::new(&file_holder, &display, 0)?;

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
        aspect_ratio: initial_win_dim.0 as f32 / initial_win_dim.1 as f32,
    };

    let mut st = State {
        file_holder: file_holder,
        model_data: model_data,
        eye: eye,
        display: display,
        program: program,
        default_texture: default_texture,
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

    let mut mouse = MouseState {
        pos: (0,0),
        grabbed: GrabState::NotGrabbed,
    };

    let mut move_dir = vec3(0.0, 0.0, 0.0);

    let mut cur_time = time::precise_time_s();
    let mut last_time;
    let mut last_anim_time = cur_time;

    let window = st.display.get_window().unwrap();

    loop {
        let (w,h) = window.get_inner_size_pixels().unwrap();
        let (w,h) = (w as i32, h as i32);
        st.eye.aspect_ratio = w as f32 / h as f32;

        let mut target = st.display.draw();

        let middle_grey = (0.4666, 0.4666, 0.4666, 1.0);
        target.clear_color_srgb_and_depth(middle_grey, 1.0);

        let mat = st.eye.model_view_persp();

        for call in st.model_data.geo_data.draw_calls.iter() {
            let model = st.model();

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

        last_time = cur_time;
        cur_time = time::precise_time_s() ;
        let dt = (cur_time - last_time) as f32;

        for ev in st.display.poll_events() {
            use glium::glutin::Event as Ev;
            use glium::glutin::ElementState as Es;
            use glium::glutin::VirtualKeyCode as K;
            match ev {
                Ev::Closed => return Ok(()),

                Ev::KeyboardInput(Es::Pressed, _, Some(K::W)) => move_dir.x = 1.0,
                Ev::KeyboardInput(Es::Pressed, _, Some(K::S)) => move_dir.x = -1.0,
                Ev::KeyboardInput(Es::Pressed, _, Some(K::A)) => move_dir.y = -1.0,
                Ev::KeyboardInput(Es::Pressed, _, Some(K::D)) => move_dir.y = 1.0,
                Ev::KeyboardInput(Es::Pressed, _, Some(K::F)) => move_dir.z = -1.0,
                Ev::KeyboardInput(Es::Pressed, _, Some(K::R)) => move_dir.z = 1.0,

                Ev::KeyboardInput(Es::Released, _, Some(K::W)) => move_dir.x = 0.0,
                Ev::KeyboardInput(Es::Released, _, Some(K::S)) => move_dir.x = 0.0,
                Ev::KeyboardInput(Es::Released, _, Some(K::A)) => move_dir.y = 0.0,
                Ev::KeyboardInput(Es::Released, _, Some(K::D)) => move_dir.y = 0.0,
                Ev::KeyboardInput(Es::Released, _, Some(K::F)) => move_dir.z = 0.0,
                Ev::KeyboardInput(Es::Released, _, Some(K::R)) => move_dir.z = 0.0,

                Ev::MouseInput(Es::Pressed, glium::glutin::MouseButton::Left) => {
                    mouse.grabbed = GrabState::Grabbed { saved_pos: mouse.pos };
                    mouse.set_position(&window, (w/2, h/2));
                    let _ = window.set_cursor_state(glium::glutin::CursorState::Hide);
                }
                Ev::MouseInput(Es::Released, glium::glutin::MouseButton::Left) => {
                    if let GrabState::Grabbed { saved_pos } = mouse.grabbed {
                        let _ = window.set_cursor_state(glium::glutin::CursorState::Normal);
                        mouse.set_position(&window, saved_pos);
                    }
                    mouse.grabbed = GrabState::NotGrabbed;
                }
                Ev::MouseMoved(x,y) => {
                    let last_pos = mouse.pos;
                    mouse.pos = (x,y);
                    if let GrabState::Grabbed { .. } = mouse.grabbed {
                        let (dx, dy) = (x - last_pos.0, y - last_pos.1);

                        // Warping the mouse (with set_position) appears to generate
                        // these MouseMoved events. In particular, the initial warp to
                        // the center of the window can generate a large displacement
                        // that makes the camera jump. Since there's no real way to tell
                        // which events are caused by warps and which are "real", we
                        // sovle this issue by just ignoring large displacements.
                        let ignore_cutoff = 20;
                        let ignore = dx.abs() > ignore_cutoff || dy.abs() > ignore_cutoff;

                        if !ignore {
                            let dv = 0.01 * vec2(dx as f32, dy as f32);
                            st.eye.free_look(dv);
                            mouse.set_position(&window, (w/2, h/2));
                        }
                    }
                }

                Ev::Focused(false) => {
                    let _ = window.set_cursor_state(glium::glutin::CursorState::Normal);
                    mouse.grabbed = GrabState::NotGrabbed;
                    move_dir = vec3(0.0, 0.0, 0.0);
                }

                Ev::KeyboardInput(Es::Pressed, _, Some(K::Comma)) => {
                    st.model_data.prev_model(&st.file_holder, &st.display)?;
                }
                Ev::KeyboardInput(Es::Pressed, _, Some(K::Period)) => {
                    st.model_data.next_model(&st.file_holder, &st.display)?;
                }
                Ev::KeyboardInput(Es::Pressed, _, Some(K::LBracket)) => {
                    st.model_data.prev_anim(&st.file_holder)?;
                    last_anim_time = cur_time;
                }
                Ev::KeyboardInput(Es::Pressed, _, Some(K::RBracket)) => {
                    st.model_data.next_anim(&st.file_holder)?;
                    last_anim_time = cur_time;
                }

                _ => ()
            }
        }

        if let Some(_) = st.model_data.anim_data {
            let frame_length = 1.0 / 60.0; // 60 fps
            let mut time_since_last_frame = cur_time - last_anim_time;
            if time_since_last_frame > frame_length {
                while time_since_last_frame > frame_length {
                    st.model_data.next_anim_frame(&st.file_holder)?;
                    time_since_last_frame -= frame_length;
                }
                last_anim_time = cur_time;
            }
        }

        let mag = move_dir.magnitude();
        if mag != 0.0 {
            let vel = dt * move_dir / mag;
            st.eye.move_by(vel);
        }
    }
}
