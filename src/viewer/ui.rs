use cgmath::{InnerSpace, Point3, vec2, vec3, Vector3};
use errors::Result;
use glium;
use glium::Surface;
use glium::glutin::EventsLoop;
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

pub struct Ui {
    db: Database,
    ctx: GlContext,
    events_loop: EventsLoop,
    view_state: ViewState,
    drawing_data: DrawingData,
    win_title: String,
    mouse: MouseState,
    fps_tracker: FpsTracker,
    move_dir: Vector3<f32>,
    move_speed: SpeedLevel,
}

impl Ui {
    pub fn new(db: Database) -> Result<Ui> {
        assert!(!db.models.is_empty());

        let events_loop = EventsLoop::new();
        let window = glium::glutin::WindowBuilder::new()
            .with_dimensions(512, 384); // 2x DS resolution
        let context = glium::glutin::ContextBuilder::new()
            .with_depth_buffer(24);
        let display = glium::Display::new(window, context, &events_loop)?;
        let ctx = GlContext::new(display)?;

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
            events_loop,
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
        // Time when the animation last changed; None if we aren't animating
        let mut last_anim_time: Option<f64> = None;

        loop {
            self.update_title();
            self.update_aspect_ratio();

            self.draw_frame();

            self.fps_tracker.notify_frame();

            last_time = cur_time;
            cur_time = time::precise_time_s() ;
            let dt = (cur_time - last_time) as f32;

            self.fps_tracker.update(cur_time);

            {
                // HACK HACK HACK: borrow these so bottowck knows we're not going
                // to mutate self.events_loop while its borrowed for poll_events.
                let db = &self.db;
                let ctx = &self.ctx;
                let move_dir = &mut self.move_dir;
                let move_speed = &mut self.move_speed;
                let view_state = &mut self.view_state;
                let mouse = &mut self.mouse;

                let mut should_close = false;

                self.events_loop.poll_events(|ev| {
                    use glium::glutin::Event as Ev;
                    use glium::glutin::WindowEvent as WEv;

                    // By the way, according to the docs we should probably be using
                    // DeviceEvents instead of these WindowEvents (it specifically
                    // calls out first-person cameras lol). But I don't know how to
                    // do it. For example, DeviceEvent::MouseMotion has a mouse
                    // delta in an unspecified coordinate system--so what good is
                    // it?!

                    // TODO handle touch events

                    let win_ev = match ev {
                        Ev::WindowEvent { event, .. } => event,
                        _ => return,
                    };
                    match win_ev {
                        WEv::Closed => {
                            should_close = true;
                        }
                        WEv::KeyboardInput { input, .. } => {
                            use glium::glutin::ElementState::{Pressed, Released};
                            use glium::glutin::VirtualKeyCode as K;

                            if input.virtual_keycode.is_none() { return; }
                            let keycode = input.virtual_keycode.unwrap();

                            match (input.state, keycode) {
                                // WASD Movement
                                (Pressed, K::W) => move_dir.x =  1.0,
                                (Pressed, K::S) => move_dir.x = -1.0,
                                (Pressed, K::D) => move_dir.y =  1.0,
                                (Pressed, K::A) => move_dir.y = -1.0,
                                (Pressed, K::E) => move_dir.z =  1.0,
                                (Pressed, K::Q) => move_dir.z = -1.0,
                                (Released, K::W) => move_dir.x = 0.0,
                                (Released, K::D) => move_dir.y = 0.0,
                                (Released, K::S) => move_dir.x = 0.0,
                                (Released, K::A) => move_dir.y = 0.0,
                                (Released, K::Q) => move_dir.z = 0.0,
                                (Released, K::E) => move_dir.z = 0.0,

                                // Change speed
                                (Pressed, K::LShift) => move_speed.speed_up(),
                                (Pressed, K::LControl) => move_speed.speed_down(),

                                // Change model
                                (Pressed, K::Comma) =>
                                    view_state.advance_model(db, Dir::Prev),
                                (Pressed, K::Period) =>
                                    view_state.advance_model(db, Dir::Next),

                                // Change animation
                                (Pressed, K::O) =>
                                    view_state.advance_anim(db, Dir::Prev),
                                (Pressed, K::P) =>
                                    view_state.advance_anim(db, Dir::Next),

                                _ => ()
                            }
                        }
                        WEv::MouseInput { state, button, .. } => {
                            use glium::glutin::ElementState as Es;
                            use glium::glutin::MouseButton as MB;

                            let window = ctx.display.gl_window();

                            match (state, button) {
                                (Es::Pressed, MB::Left) => {
                                    mouse.grabbed =
                                        GrabState::Grabbed { saved_pos: mouse.pos };
                                    let (w,h) = window.get_inner_size().unwrap();
                                    let center = (w as f64 / 2.0, h as f64 / 2.0);
                                    mouse.set_position(&window, center);
                                    let _ = window.set_cursor_state(glium::glutin::CursorState::Hide);
                                }
                                (Es::Released, MB::Left) => {
                                    if let GrabState::Grabbed { saved_pos } = mouse.grabbed {
                                        let _ = window.set_cursor_state(glium::glutin::CursorState::Normal);
                                        mouse.set_position(&window, saved_pos);
                                    }
                                    mouse.grabbed = GrabState::NotGrabbed;
                                }
                                _ => ()
                            }
                        }
                        WEv::CursorMoved { position : (x, y), .. } => {
                            let last_pos = mouse.pos;
                            mouse.pos = (x, y);

                            if let GrabState::Grabbed { .. } = mouse.grabbed {
                                let (dx, dy) = (x - last_pos.0, y - last_pos.1);

                                // Warping the mouse (with set_position) appears to generate
                                // these mouse motion events. In particular, the initial warp to
                                // the center of the window can generate a large displacement
                                // that makes the camera jump. Since there's no real way to tell
                                // which events are caused by warps and which are "real", we
                                // solve this issue by just ignoring large displacements.
                                let ignore_cutoff = 20.0;
                                let ignore = dx.abs() > ignore_cutoff || dy.abs() > ignore_cutoff;

                                if !ignore {
                                    let dv = 0.01 * vec2(dx as f32, dy as f32);
                                    view_state.eye.free_look(dv);
                                }

                                let window = ctx.display.gl_window();
                                let (w, h) = match window.get_inner_size() {
                                    Some(dim) => dim,
                                    None => return,
                                };
                                let center = (w as f64 / 2.0, h as f64 / 2.0);
                                mouse.set_position(&window, center);
                            }
                        }
                        WEv::Focused(false) => {
                            // Process loss of window focus. Try to release any grab
                            // and stop moving.

                            let window = ctx.display.gl_window();

                            let _ = window.set_cursor_state(glium::glutin::CursorState::Normal);
                            mouse.grabbed = GrabState::NotGrabbed;
                            *move_dir = vec3(0.0, 0.0, 0.0);
                        }
                        _ => ()
                    }
                });

                if should_close { return; }
            }

            if self.view_state.anim_state.is_some() {
                if last_anim_time.is_none() {
                    last_anim_time = Some(cur_time);
                }
                let last_anim_time = last_anim_time.as_mut().unwrap();

                let frame_length = 1.0 / 60.0; // 60 fps
                let mut time_since_last_frame = cur_time - *last_anim_time;
                if time_since_last_frame > frame_length {
                    while time_since_last_frame > frame_length {
                        self.view_state.next_frame(&self.db);
                        time_since_last_frame -= frame_length;
                    }
                    *last_anim_time = cur_time;
                }
            } else {
                last_anim_time = None;
            }

            // Move the camera
            let mag = self.move_dir.magnitude();
            if mag != 0.0 {
                let vel = self.move_speed.speed() * (self.move_dir / mag);
                self.view_state.eye.move_by(dt * vel);
            }
        }
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
            self.drawing_data.draw(&self.db, &self.ctx, &mut target);
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

        let window = self.ctx.display.gl_window();
        window.set_title(&self.win_title);
    }

    fn update_aspect_ratio(&mut self) {
        let window = self.ctx.display.gl_window();
        let (w, h) = window.get_inner_size().unwrap();
        self.view_state.eye.aspect_ratio = w as f32 / h as f32;
    }
}

fn print_controls() {
    print!(concat!(
        "--------\n",
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
