//mod draw;
//mod fps;
//mod gl_context;
//mod mouse;
//mod speed;
//mod state;
mod model_viewer;
//mod eye;
mod main_loop;
mod viewer;
mod fps;

use clap::ArgMatches;
use db::Database;
use connection::{Connection, ConnectionOptions};
use errors::Result;

/// Initial window width.
pub static WINDOW_WIDTH: u32 = 640;
/// Initial window height.
pub static WINDOW_HEIGHT: u32 = 480;
/// Window background color.
pub static BG_COLOR: (f32, f32, f32, f32) = (0.3, 0.3, 0.3, 1.0);
/// Near-plane distance for perspective.
pub static Z_NEAR: f32 = 0.01;
/// Far-plane distance for perspective.
pub static Z_FAR: f32 = 4000.0;
/// Vertical field-of-view for perspective (radians).
pub static FOV_Y: f32 = 1.1;
/// Animation framerate (seconds/frame)
pub static FRAMERATE: f64 = 1.0 / 60.0;
/// Calculate FPS over intervals of this length (seconds).
pub static FPS_INTERVAL: f64 = 2.0;

pub fn main(matches: &ArgMatches) -> Result<()> {
    let db = Database::from_arg_matches(matches)?;
    db.print_status();

    if db.models.len() == 0 {
        println!("No models, nothing to do!");
        return Ok(());
    }

    let conn_options = ConnectionOptions::from_arg_matches(matches);
    let conn = Connection::build(&db, conn_options);

    // Print the controls
    println!("{}", viewer::CONTROL_HELP);

    main_loop::main_loop(db, conn);

    Ok(())
}
