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

    let num_models = db.models.len();
    let num_animations = db.animations.len();

    let plural = |x| if x != 1 { "s" } else { "" };
    println!("Found {} model{}.", num_models, plural(num_models));
    println!("Found {} animation{}.", num_animations, plural(num_animations));

    if num_models == 0 {
        println!("Nothing to do.");
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
