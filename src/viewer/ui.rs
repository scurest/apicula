use cgmath::{InnerSpace, Point3, vec2, vec3, Vector3};
use errors::Result;
use glium;
use glium::Surface;
use std::default::Default;
use std::fmt::Write;
use time;
use viewer::draw::DrawingData;
use viewer::eye::Eye;
use viewer::fps::FpsTracker;
use viewer::gl_context::GlContext;
use viewer::mouse::GrabState;
use viewer::mouse::MouseState;
use viewer::speed::SpeedLevel;
use viewer::state::Dir;
use viewer::state::ViewState;
use db::Database;

pub struct Ui<'a> {
    db: Database,
    ctx: &'a GlContext,

    view_state: ViewState,
    drawing_data: DrawingData,

    win_title: String,
    mouse: MouseState,
    fps_tracker: FpsTracker,
    move_dir: Vector3<f32>,
    move_speed: SpeedLevel,
}

type KeyEvent = (glium::glutin::ElementState, glium::glutin::VirtualKeyCode);
type MouseEvent = (glium::glutin::ElementState, glium::glutin::MouseButton);

impl<'a> Ui<'a> {
    pub fn new(db: Database, ctx: &'a GlContext) -> Result<Ui<'a>> {
        assert!(!db.models.is_empty());

        // Initial position at the origin, viewing the first
        // model in its bind pose.
        let eye = Eye {
            position: Point3::new(0.0, 0.0, 0.0),
            azimuth: 0.0,
            altitude: 0.0,
            aspect_ratio: 0.0,
        };
        let view_state = ViewState {
            model_id: 0,
            anim_state: None,
            eye,
        };

        let drawing_data =
            DrawingData::from_view_state(&ctx.display, &db, &view_state);

        let win_title = String::new();
        let mouse = MouseState::new();
        let fps_tracker = FpsTracker::new();
        let move_dir = vec3(0.0, 0.0, 0.0);
        let move_speed = Default::default();

        Ok(Ui {
            db,
            ctx,
            view_state,
            drawing_data,
            win_title,
            mouse,
            fps_tracker,
            move_dir,
            move_speed,
        })
    }

    pub fn run(&mut self) {
        print_controls();

        let mut cur_time = time::precise_time_s();
        let mut last_time;
        let mut last_anim_time = cur_time;

        loop {
            self.update_title();
            self.update_aspect_ratio();

            self.draw_frame();

            self.fps_tracker.notify_frame();

            last_time = cur_time;
            cur_time = time::precise_time_s() ;
            let dt = (cur_time - last_time) as f32;

            self.fps_tracker.update(cur_time);

            for ev in self.ctx.display.poll_events() {
                use glium::glutin::Event as Ev;
                match ev {
                    Ev::Closed => return,
                    Ev::KeyboardInput(press_state, _, Some(keycode)) =>
                        self.process_key((press_state, keycode)),
                    Ev::MouseInput(press_state, button) =>
                        self.process_mouse_button((press_state, button)),
                    Ev::MouseMoved(x, y) =>
                        self.process_mouse_motion((x,y)),
                    Ev::Focused(false) =>
                        self.process_blur(),
                    _ => ()
                }
            }

            if self.view_state.anim_state.is_some() {
                let frame_length = 1.0 / 60.0; // 60 fps
                let mut time_since_last_frame = cur_time - last_anim_time;
                if time_since_last_frame > frame_length {
                    while time_since_last_frame > frame_length {
                        self.view_state.next_frame(&self.db);
                        time_since_last_frame -= frame_length;
                    }
                    last_anim_time = cur_time;
                }
            }

            // Move the camera
            let mag = self.move_dir.magnitude();
            if mag != 0.0 {
                let vel = self.move_speed.speed() * (self.move_dir / mag);
                self.view_state.eye.move_by(dt * vel);
            }
        }
    }

    fn process_key(&mut self, ev: KeyEvent) {
        use glium::glutin::ElementState as Es;
        use glium::glutin::VirtualKeyCode as K;

        match ev {
            // WASD Movement
            (Es::Pressed, K::W) => self.move_dir.x =  1.0,
            (Es::Pressed, K::S) => self.move_dir.x = -1.0,
            (Es::Pressed, K::D) => self.move_dir.y =  1.0,
            (Es::Pressed, K::A) => self.move_dir.y = -1.0,
            (Es::Pressed, K::E) => self.move_dir.z =  1.0,
            (Es::Pressed, K::Q) => self.move_dir.z = -1.0,
            (Es::Released, K::W) => self.move_dir.x = 0.0,
            (Es::Released, K::D) => self.move_dir.y = 0.0,
            (Es::Released, K::S) => self.move_dir.x = 0.0,
            (Es::Released, K::A) => self.move_dir.y = 0.0,
            (Es::Released, K::Q) => self.move_dir.z = 0.0,
            (Es::Released, K::E) => self.move_dir.z = 0.0,

            // Change speed
            (Es::Pressed, K::LShift) => self.move_speed.speed_up(),
            (Es::Pressed, K::LControl) => self.move_speed.speed_down(),

            // Change model
            (Es::Pressed, K::Comma) =>
                self.view_state.advance_model(&self.db, Dir::Prev),
            (Es::Pressed, K::Period) =>
                self.view_state.advance_model(&self.db, Dir::Next),

            // Change animation
            (Es::Pressed, K::O) =>
                self.view_state.advance_anim(&self.db, Dir::Prev),
            (Es::Pressed, K::P) =>
                self.view_state.advance_anim(&self.db, Dir::Next),

            _ => ()
        }
    }

    fn process_mouse_button(&mut self, ev: MouseEvent) {
        use glium::glutin::ElementState as Es;
        use glium::glutin::MouseButton as MB;

        let window = self.ctx.display.get_window().unwrap();

        match ev {
            (Es::Pressed, MB::Left) => {
                self.mouse.grabbed =
                    GrabState::Grabbed { saved_pos: self.mouse.pos };
                let (w,h) = window.get_inner_size_pixels().unwrap();
                let center = (w as i32 / 2, h as i32 / 2);
                self.mouse.set_position(&window, center);
                let _ = window.set_cursor_state(glium::glutin::CursorState::Hide);
            }
            (Es::Released, MB::Left) => {
                if let GrabState::Grabbed { saved_pos } = self.mouse.grabbed {
                    let _ = window.set_cursor_state(glium::glutin::CursorState::Normal);
                    self.mouse.set_position(&window, saved_pos);
                }
                self.mouse.grabbed = GrabState::NotGrabbed;
            }
            _ => ()
        }
    }

    fn process_mouse_motion(&mut self, (x, y): (i32, i32)) {
        let last_pos = self.mouse.pos;
        self.mouse.pos = (x,y);

        if let GrabState::Grabbed { .. } = self.mouse.grabbed {
            let (dx, dy) = (x - last_pos.0, y - last_pos.1);

            // Warping the mouse (with set_position) appears to generate
            // these mouse motion events. In particular, the initial warp to
            // the center of the window can generate a large displacement
            // that makes the camera jump. Since there's no real way to tell
            // which events are caused by warps and which are "real", we
            // solve this issue by just ignoring large displacements.
            let ignore_cutoff = 20;
            let ignore = dx.abs() > ignore_cutoff || dy.abs() > ignore_cutoff;

            if !ignore {
                let dv = 0.01 * vec2(dx as f32, dy as f32);
                self.view_state.eye.free_look(dv);
            }

            let window = self.ctx.display.get_window().unwrap();
            let (w,h) = window.get_inner_size_pixels().unwrap();
            let center = (w as i32 / 2, h as i32 / 2);
            self.mouse.set_position(&window, center);
        }
    }

    /// Process loss of window focus. Try to release any grab and stop moving.
    fn process_blur(&mut self) {
        let window = self.ctx.display.get_window().unwrap();

        let _ = window.set_cursor_state(glium::glutin::CursorState::Normal);
        self.mouse.grabbed = GrabState::NotGrabbed;
        self.move_dir = vec3(0.0, 0.0, 0.0);
    }

    fn draw_frame(&mut self) {
        self.drawing_data.change_view_state(&self.ctx.display, &self.db, &self.view_state);

        let mut target = self.ctx.display.draw();

        if self.drawing_data.has_error() {
            let red = (1.0, 0.0, 0.0, 1.0);
            target.clear_color_srgb_and_depth(red, 1.0);
        } else {
            let middle_grey = (0.4666, 0.4666, 0.4666, 1.0);
            target.clear_color_srgb_and_depth(middle_grey, 1.0);

            let draw_params = glium::DrawParameters {
                depth: glium::Depth {
                    test: glium::draw_parameters::DepthTest::IfLess,
                    write: true,
                    .. Default::default()
                },
                backface_culling: glium::draw_parameters::BackfaceCullingMode::CullClockwise,
                .. Default::default()
            };

            self.drawing_data.draw(
                &self.db,
                self.ctx,
                &mut target,
                &draw_params,
            );
        }

        target.finish().unwrap();
    }

    fn update_title(&mut self) {
        self.win_title.clear();

        if self.drawing_data.has_error() {
            write!(&mut self.win_title, "{{ERROR}} ").unwrap();
        };
        let model = &self.db.models[self.view_state.model_id];
        write!(&mut self.win_title, "{model_name}[{model_num}/{num_models}] === ",
            model_name = model.name,
            model_num = self.view_state.model_id + 1,
            num_models = self.db.models.len(),
        ).unwrap();
        if let Some(ref anim_state) = self.view_state.anim_state {
            let anim = &self.db.animations[anim_state.anim_id];
            write!(&mut self.win_title, "{anim_name}[{anim_num}/{num_anims}] ({cur_frame}/{num_frames}) === ",
                anim_name = anim.name,
                anim_num = anim_state.anim_id + 1,
                num_anims = self.db.animations.len(),
                cur_frame = anim_state.cur_frame + 1,
                num_frames = anim.num_frames,
            ).unwrap()
        } else {
            write!(&mut self.win_title, "Bind Pose === ").unwrap()
        }
        let fps = self.fps_tracker.get_fps();
        write!(&mut self.win_title, "{:5.2}fps", fps).unwrap();

        let window = self.ctx.display.get_window().unwrap();
        window.set_title(&self.win_title);
    }

    fn update_aspect_ratio(&mut self) {
        let window = self.ctx.display.get_window().unwrap();
        let (w, h) = window.get_inner_size_pixels().unwrap();
        self.view_state.eye.aspect_ratio = w as f32 / h as f32;
    }
}

fn print_controls() {
    print!(concat!(
        "Controls\n",
        "--------\n",
        "  WASD         Forward/Left/Back/Right\n",
        "  EQ           Up/Down\n",
        "  L.Shift      Increase Speed\n",
        "  L.Ctrl       Decrease Speed\n",
        "  L.Mouse      Free Look\n",
        "  OP           Prev/Next Animation\n",
        "  ,.           Prev/Next Model\n",
    ));
}
