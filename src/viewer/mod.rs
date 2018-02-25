mod draw;
mod eye;
mod fps;
mod gl_context;
mod mouse;
mod speed;
mod state;
mod ui;

use clap::ArgMatches;
use errors::Result;
use viewer::ui::Ui;
use glium;
use viewer::gl_context::GlContext;
use db::Database;

pub fn main(matches: &ArgMatches) -> Result<()> {
    let db = Database::from_arg_matches(matches)?;

    db.print_status();

    if db.models.is_empty() {
        println!("No models. Nothing to do.\n");
        return Ok(())
    }

    use glium::DisplayBuild;
    let display = glium::glutin::WindowBuilder::new()
        .with_dimensions(512, 384) // 2x DS resolution
        .with_depth_buffer(24)
        .build_glium()
        .unwrap();
    let ctx = GlContext::new(display)?;

    let mut ui = Ui::new(db, &ctx)?;
    ui.run();

    Ok(())
}
