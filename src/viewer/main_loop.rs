use glium::glutin::{self, dpi::{LogicalSize, PhysicalSize, PhysicalPosition}};
use glium::glutin::event_loop::ControlFlow;
use super::viewer::Viewer;
use db::Database;
use connection::Connection;

pub fn main_loop(db: Database, conn: Connection) {
    let window_builder = glutin::window::WindowBuilder::new()
        .with_inner_size(LogicalSize {
            width: super::WINDOW_WIDTH as f64,
            height: super::WINDOW_HEIGHT as f64
        });
    let context_builder = glutin::ContextBuilder::new()
        .with_depth_buffer(24);
    let events_loop = glutin::event_loop::EventLoop::new();
    let display = glium::Display::new(window_builder, context_builder, &events_loop)
        .expect("failed to get rendering context");

    let mut viewer = Viewer::new(&display, db, conn);

    struct State {
        last_mouse_xy: PhysicalPosition<f64>,
        saved_mouse_xy: PhysicalPosition<f64>,
        mouse_grabbed: bool,
        win_title: String,
        cur_time: u64,
        last_time: u64,
    };

    let mut state = State {
        last_mouse_xy: PhysicalPosition { x: 0.0, y: 0.0 },
        saved_mouse_xy: PhysicalPosition { x: 0.0, y: 0.0 },
        mouse_grabbed: false,
        win_title: String::with_capacity(512),
        cur_time: time::precise_time_ns(),
        last_time: time::precise_time_ns(),
    };

    events_loop.run(move |ev, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        let gl_window = display.gl_window();
        let window = gl_window.window();

        state.last_time = state.cur_time;
        state.cur_time = time::precise_time_ns();
        let dt_in_ns = state.cur_time.wrapping_sub(state.last_time);
        let dt = dt_in_ns as f64 / 1_000_000_000.0;

        viewer.update(&display, dt);

        let PhysicalSize { width, height } = window.inner_size();
        if width > 0 && height > 0 {
            viewer.set_aspect_ratio(width as f64 / height as f64);
        }

        let mut frame = display.draw();
        viewer.draw(&mut frame);
        frame.finish().expect("rendering error");

        state.win_title.clear();
        viewer.title(&mut state.win_title);
        window.set_title(&state.win_title);

        use self::glutin::event::Event as Ev;
        use self::glutin::event::WindowEvent as WEv;
        use self::glutin::event::DeviceEvent as DEv;

        match ev {
            Ev::WindowEvent { event, .. } => match event {
                WEv::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                WEv::KeyboardInput { input, .. } => {
                    if input.virtual_keycode.is_none() { return; }
                    let keycode = input.virtual_keycode.unwrap();
                    viewer.key(&display, (input.state, keycode, input.modifiers));
                }
                WEv::MouseInput { state: mouse_state, button, .. } => {
                    use self::glutin::event::ElementState as Es;
                    use self::glutin::event::MouseButton as MB;

                    match (mouse_state, button) {
                        (Es::Pressed, MB::Left) => {
                            state.mouse_grabbed = true;
                            state.saved_mouse_xy = state.last_mouse_xy;
                            let _ = window.set_cursor_grab(true);
                            let _ = window.set_cursor_visible(true);
                        }
                        (Es::Released, MB::Left) => {
                            state.mouse_grabbed = false;
                            let _ = window.set_cursor_position(state.saved_mouse_xy);
                            let _ = window.set_cursor_grab(false);
                            let _ = window.set_cursor_visible(false);
                        }
                        _ => (),
                    }
                }
                WEv::CursorMoved { position, .. } => {
                    state.last_mouse_xy = position;

                    if state.mouse_grabbed {
                        // Warp the mouse to the center of the window to
                        // keep it inside our window to fake mouse capture.
                        let PhysicalSize { width, height } = window.outer_size();
                        let center = PhysicalPosition {
                            x: width as f64 / 2.0,
                            y: height as f64 / 2.0,
                        };
                        let _ = window.set_cursor_position(center);
                    }
                }
                WEv::Focused(false) => {
                    viewer.blur();

                    // Release the mouse
                    state.mouse_grabbed = false;
                    let _ = window.set_cursor_grab(false);
                    let _ = window.set_cursor_visible(false);
                }
                _ => ()
            },
            Ev::DeviceEvent { event, .. } => match event {
                DEv::MouseMotion { delta } => {
                    // delta is in an "unspecified coordinate system" but
                    // appears to be pixels on my machine
                    if state.mouse_grabbed {
                        viewer.mouse_drag(delta);
                    }
                }
                _ => (),
            },
            _ => (),
        }
    });
}
