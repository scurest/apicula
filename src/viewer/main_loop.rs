use glium::winit;
use winit::dpi::{PhysicalSize, PhysicalPosition};
use winit::keyboard::ModifiersState;
use super::viewer::Viewer;
use crate::db::Database;
use crate::connection::Connection;

pub fn main_loop(db: Database, conn: Connection) {
    let event_loop = winit::event_loop::EventLoop::builder()
        .build()
        .expect("event loop building");
    let (window, display) = glium::backend::glutin::SimpleWindowBuilder::new()
        .with_inner_size(super::WINDOW_WIDTH, super::WINDOW_HEIGHT)
        .with_vsync(true)
        .build(&event_loop);

    let mut viewer = Viewer::new(&display, db, conn);

    struct State {
        last_mouse_xy: PhysicalPosition<f64>,
        mouse_grabbed: bool,
        modifiers: ModifiersState,
        win_title: String,
        cur_time: u64,
        last_time: u64,
    }

    let mut state = State {
        last_mouse_xy: PhysicalPosition { x: 0.0, y: 0.0 },
        mouse_grabbed: false,
        modifiers: Default::default(),
        win_title: String::with_capacity(512),
        cur_time: time::precise_time_ns(),
        last_time: time::precise_time_ns(),
    };

    let _ = event_loop.run(move |ev, window_target| {
        use winit::event::Event as Ev;
        use winit::event::WindowEvent as WEv;
        use winit::event::DeviceEvent as DEv;

        match ev {
            Ev::WindowEvent { event, .. } => match event {
                WEv::RedrawRequested => {
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
                }
                WEv::CloseRequested => {
                    window_target.exit();
                }
                WEv::KeyboardInput { event: e, .. } => {
                    if let winit::keyboard::PhysicalKey::Code(code) = e.physical_key {
                        viewer.key(
                            &display,
                            code,
                            e.state == winit::event::ElementState::Pressed,
                            state.modifiers,
                        );
                    }
                }
                WEv::ModifiersChanged(m) => {
                    state.modifiers = m.state();
                }
                WEv::MouseInput { state: mouse_state, button, .. } => {
                    use winit::event::ElementState as Es;
                    use winit::event::MouseButton as MB;

                    match (mouse_state, button) {
                        (Es::Pressed, MB::Left) => {
                            state.mouse_grabbed = true;
                            let _ = window.set_cursor_grab(winit::window::CursorGrabMode::Locked);
                            window.set_cursor_visible(false);
                        }
                        (Es::Released, MB::Left) => {
                            state.mouse_grabbed = false;
                            let _ = window.set_cursor_grab(winit::window::CursorGrabMode::None);
                            window.set_cursor_visible(true);
                        }
                        _ => (),
                    }
                }
                WEv::CursorMoved { position, .. } => {
                    state.last_mouse_xy = position;
                }
                WEv::Focused(false) => {
                    viewer.blur();

                    // Release the mouse
                    state.mouse_grabbed = false;
                    let _ = window.set_cursor_grab(winit::window::CursorGrabMode::None);
                    window.set_cursor_visible(true);
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
            Ev::AboutToWait => {
                window.request_redraw();
            },
            _ => (),
        }
    });
}
