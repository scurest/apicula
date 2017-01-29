use cgmath::InnerSpace;
use cgmath::Point3;
use cgmath::vec2;
use cgmath::vec3;
use errors::Result;
use files::FileHolder;
use geometry::Vertex;
use glium;
use glium::backend::glutin_backend::WinRef;
use glium::Surface;
use std::fmt::Write;
use time;
use viewer::eye::Eye;
use viewer::state::ModelData;
use viewer::state::State;

implement_vertex!(Vertex, position, texcoord, color);

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

struct FpsTracker {
    last_fps: f64,
    last_fps_update_time: f64,
    frames_since_last_update: u32,
}

impl FpsTracker {
    /// Give the FPS tracker the chance to update itself.
    fn tick(&mut self, cur_time: f64) {
        let time_since_last_fps_update = cur_time - self.last_fps_update_time;
        if time_since_last_fps_update >= 2.0 { // Update every two seconds
            self.last_fps =
                self.frames_since_last_update as f64 / time_since_last_fps_update;
            self.frames_since_last_update = 0;
            self.last_fps_update_time = cur_time;
        }
    }
}

pub fn viewer(file_holder: FileHolder) -> Result<()> {
    let num_models = file_holder.models.len();
    let num_animations = file_holder.animations.len();
    let suf = |x| if x != 1 { "s" } else { "" };
    println!("Found {} model{}.", num_models, suf(num_models));
    println!("Found {} animation{}.", num_animations, suf(num_animations));

    if num_models == 0 {
        println!("Nothing to do.");
        return Ok(())
    }

    let initial_win_dim = (512, 384); // 2x DS resolution

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

    let vertex_shader_src = include_str!("shaders/vert.glsl");
    let fragment_shader_src = include_str!("shaders/frag.glsl");

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

    print_controls();
    run(&mut st)
}

fn print_controls() {
    print!(concat!(
        "Controls\n",
        "  WASD         Forward/Left/Back/Right\n",
        "  EQ           Up/Down\n",
        "  Left Shift   Move Faster\n",
        "  Left Mouse   Free Look\n",
        "  OP           Prev/Next Animation\n",
        "  ,.           Prev/Next Model\n",
    ));
}

fn write_title(s: &mut String, st: &State, fps: f64) {
    write!(s, "{model_name}[{model_num}/{num_models}] === ",
        model_name = st.model().name,
        model_num = st.model_data.model_index() + 1,
        num_models = st.file_holder.models.len(),
    ).unwrap();
    if let Some(anim_data) = st.model_data.animation_data() {
        let anim = &st.file_holder.animations[anim_data.index];
        write!(s, "{anim_name}[{anim_num}/{num_anims}] ({cur_frame}/{max_frame}) === ",
            anim_name = anim.name,
            anim_num = anim_data.index + 1,
            num_anims = st.file_holder.animations.len(),
            cur_frame = anim_data.cur_frame,
            max_frame = anim.num_frames,
        ).unwrap()
    } else {
        write!(s, "Bind Pose === ").unwrap()
    }
    write!(s, "{:5.2}fps", fps).unwrap();
}

fn run(st: &mut State) -> Result<()> {
    let draw_params = glium::DrawParameters {
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
    let mut speed = 1.0;

    let mut cur_time = time::precise_time_s();
    let mut last_time;
    let mut last_anim_time = cur_time;

    let mut fps_tracker = FpsTracker {
        last_fps: 0.0,
        last_fps_update_time: cur_time,
        frames_since_last_update: 0,
    };

    let window = st.display.get_window().unwrap();

    let mut title = String::new();

    loop {
        title.clear();
        write_title(&mut title, st, fps_tracker.last_fps);
        window.set_title(&title);

        let (w,h) = window.get_inner_size_pixels().unwrap();
        let (w,h) = (w as i32, h as i32);
        st.eye.aspect_ratio = w as f32 / h as f32;

        let mut target = st.display.draw();

        let middle_grey = (0.4666, 0.4666, 0.4666, 1.0);
        target.clear_color_srgb_and_depth(middle_grey, 1.0);

        st.draw(&mut target, &draw_params)?;

        target.finish().unwrap();

        fps_tracker.frames_since_last_update += 1;

        last_time = cur_time;
        cur_time = time::precise_time_s() ;
        let dt = (cur_time - last_time) as f32;

        fps_tracker.tick(cur_time);

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
                Ev::KeyboardInput(Es::Pressed, _, Some(K::Q)) => move_dir.z = -1.0,
                Ev::KeyboardInput(Es::Pressed, _, Some(K::E)) => move_dir.z = 1.0,

                Ev::KeyboardInput(Es::Released, _, Some(K::W)) => move_dir.x = 0.0,
                Ev::KeyboardInput(Es::Released, _, Some(K::S)) => move_dir.x = 0.0,
                Ev::KeyboardInput(Es::Released, _, Some(K::A)) => move_dir.y = 0.0,
                Ev::KeyboardInput(Es::Released, _, Some(K::D)) => move_dir.y = 0.0,
                Ev::KeyboardInput(Es::Released, _, Some(K::Q)) => move_dir.z = 0.0,
                Ev::KeyboardInput(Es::Released, _, Some(K::E)) => move_dir.z = 0.0,

                Ev::KeyboardInput(Es::Pressed, _, Some(K::LShift)) => speed = 10.0,
                Ev::KeyboardInput(Es::Released, _, Some(K::LShift)) => speed = 1.0,

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
                        // solve this issue by just ignoring large displacements.
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
                    speed = 1.0;
                }

                Ev::KeyboardInput(Es::Pressed, _, Some(K::Comma)) => {
                    st.model_data.prev_model(&st.file_holder, &st.display)?;
                }
                Ev::KeyboardInput(Es::Pressed, _, Some(K::Period)) => {
                    st.model_data.next_model(&st.file_holder, &st.display)?;
                }
                Ev::KeyboardInput(Es::Pressed, _, Some(K::O)) => {
                    st.model_data.prev_anim(&st.file_holder)?;
                    last_anim_time = cur_time;
                }
                Ev::KeyboardInput(Es::Pressed, _, Some(K::P)) => {
                    st.model_data.next_anim(&st.file_holder)?;
                    last_anim_time = cur_time;
                }

                _ => ()
            }
        }

        if st.model_data.has_animation() {
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
            let vel = dt * speed * move_dir / mag;
            st.eye.move_by(vel);
        }
    }
}
