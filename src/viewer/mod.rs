mod draw;
mod eye;
mod fps;
mod gl_context;
mod mouse;
mod speed;
mod state;

use cgmath::{InnerSpace, Point3, vec2, vec3, Vector3};
use clap::ArgMatches;
use db::Database;
use errors::Result;
use glium::{self, Surface};
use glium::glutin::EventsLoop;
use std::default::Default;
use std::fmt::Write;
use time;
use viewer::draw::DrawingData;
use viewer::eye::Eye;
use viewer::fps::FpsTracker;
use viewer::gl_context::GlContext;
use viewer::mouse::{GrabState, MouseState};
use viewer::speed::SpeedLevel;
use viewer::state::{Dir, ViewState};

pub fn main(matches: &ArgMatches) -> Result<()> {
    let db = Database::from_arg_matches(matches)?;
    db.print_status();
    run_viewer(db)?;
    Ok(())
}

fn run_viewer(db: Database) -> Result<()> {
    if db.models.is_empty() {
        println!("No models. Nothing to do.\n");
        return Ok(());
    }

    print_controls();

    let mut events_loop = EventsLoop::new();
    let mut ui = Ui::new(db, &events_loop)?;

    let mut cur_time = time::precise_time_s();
    let mut last_time;
    // Time when the animation last changed; None if we aren't animating
    let mut last_anim_time: Option<f64> = None;

    loop {
        ui.update_title();
        ui.update_aspect_ratio();

        ui.draw_frame();
        ui.fps_tracker.notify_frame();

        last_time = cur_time;
        cur_time = time::precise_time_s() ;
        let dt = (cur_time - last_time) as f32;

        ui.fps_tracker.update(cur_time);

        let mut should_close = false;

        events_loop.poll_events(|ev| {
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
                        (Pressed, K::W) => ui.move_dir.x =  1.0,
                        (Pressed, K::S) => ui.move_dir.x = -1.0,
                        (Pressed, K::D) => ui.move_dir.y =  1.0,
                        (Pressed, K::A) => ui.move_dir.y = -1.0,
                        (Pressed, K::E) => ui.move_dir.z =  1.0,
                        (Pressed, K::Q) => ui.move_dir.z = -1.0,
                        (Released, K::W) => ui.move_dir.x = 0.0,
                        (Released, K::D) => ui.move_dir.y = 0.0,
                        (Released, K::S) => ui.move_dir.x = 0.0,
                        (Released, K::A) => ui.move_dir.y = 0.0,
                        (Released, K::Q) => ui.move_dir.z = 0.0,
                        (Released, K::E) => ui.move_dir.z = 0.0,

                        // Change speed
                        (Pressed, K::LShift) => ui.move_speed.speed_up(),
                        (Pressed, K::LControl) => ui.move_speed.speed_down(),

                        // Change model
                        (Pressed, K::Comma) =>
                            ui.view_state.advance_model(&ui.db, Dir::Prev),
                        (Pressed, K::Period) =>
                            ui.view_state.advance_model(&ui.db, Dir::Next),

                        // Change animation
                        (Pressed, K::O) =>
                            ui.view_state.advance_anim(&ui.db, Dir::Prev),
                        (Pressed, K::P) =>
                            ui.view_state.advance_anim(&ui.db, Dir::Next),

                        _ => ()
                    }
                }
                WEv::MouseInput { state, button, .. } => {
                    use glium::glutin::ElementState as Es;
                    use glium::glutin::MouseButton as MB;

                    let window = ui.ctx.display.gl_window();

                    match (state, button) {
                        (Es::Pressed, MB::Left) => {
                            ui.mouse.grabbed =
                                GrabState::Grabbed { saved_pos: ui.mouse.pos };
                            let (w,h) = window.get_inner_size().unwrap();
                            let center = (w as f64 / 2.0, h as f64 / 2.0);
                            ui.mouse.set_position(&window, center);
                            let _ = window.set_cursor_state(glium::glutin::CursorState::Hide);
                        }
                        (Es::Released, MB::Left) => {
                            if let GrabState::Grabbed { saved_pos } = ui.mouse.grabbed {
                                let _ = window.set_cursor_state(glium::glutin::CursorState::Normal);
                                ui.mouse.set_position(&window, saved_pos);
                            }
                            ui.mouse.grabbed = GrabState::NotGrabbed;
                        }
                        _ => ()
                    }
                }
                WEv::CursorMoved { position : (x, y), .. } => {
                    let last_pos = ui.mouse.pos;
                    ui.mouse.pos = (x, y);

                    if let GrabState::Grabbed { .. } = ui.mouse.grabbed {
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
                            ui.view_state.eye.free_look(dv);
                        }

                        let window = ui.ctx.display.gl_window();
                        let (w, h) = match window.get_inner_size() {
                            Some(dim) => dim,
                            None => return,
                        };
                        let center = (w as f64 / 2.0, h as f64 / 2.0);
                        ui.mouse.set_position(&window, center);
                    }
                }
                WEv::Focused(false) => {
                    // Process loss of window focus. Try to release any grab
                    // and stop moving.

                    let window = ui.ctx.display.gl_window();

                    let _ = window.set_cursor_state(glium::glutin::CursorState::Normal);
                    ui.mouse.grabbed = GrabState::NotGrabbed;
                    ui.move_dir = vec3(0.0, 0.0, 0.0);
                }
                _ => ()
            }
        });

        if should_close { return Ok(()); }

        if ui.view_state.anim_state.is_some() {
            if last_anim_time.is_none() {
                last_anim_time = Some(cur_time);
            }
            let last_anim_time = last_anim_time.as_mut().unwrap();

            let frame_length = 1.0 / 60.0; // 60 fps
            let mut time_since_last_frame = cur_time - *last_anim_time;
            if time_since_last_frame > frame_length {
                while time_since_last_frame > frame_length {
                    ui.view_state.next_frame(&ui.db);
                    time_since_last_frame -= frame_length;
                }
                *last_anim_time = cur_time;
            }
        } else {
            last_anim_time = None;
        }

        // Move the camera
        let mag = ui.move_dir.magnitude();
        if mag != 0.0 {
            let vel = ui.move_speed.speed() * (ui.move_dir / mag);
            ui.view_state.eye.move_by(dt * vel);
        }
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

struct Ui {
    db: Database,
    ctx: GlContext,
    view_state: ViewState,
    drawing_data: DrawingData,
    win_title: String,
    mouse: MouseState,
    fps_tracker: FpsTracker,
    move_dir: Vector3<f32>,
    move_speed: SpeedLevel,
}

impl Ui {
    fn new(db: Database, events_loop: &EventsLoop) -> Result<Ui> {
        let window = glium::glutin::WindowBuilder::new()
            .with_dimensions(512, 384); // 2x DS resolution
        let context = glium::glutin::ContextBuilder::new()
            .with_depth_buffer(24);
        let display = glium::Display::new(window, context, events_loop)?;
        let ctx = GlContext::new(display)?;

        // Initial position at the origin, viewing the first
        // model in its bind pose.
        let view_state = ViewState {
            model_id: 0,
            anim_state: None,
            eye: Eye {
                position: Point3::new(0.0, 0.0, 0.0),
                azimuth: 0.0,
                altitude: 0.0,
                aspect_ratio: 0.0,
            },
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
