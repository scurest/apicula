mod draw;
mod eye;
mod fps;
mod gl_context;
mod mouse;
mod state;
mod ui;

use clap::ArgMatches;
use errors::Result;
use files::BufferHolder;
use files::FileHolder;
use viewer::ui::Ui;
use glium;
use viewer::gl_context::GlContext;

pub fn main(matches: &ArgMatches) -> Result<()> {
    let input_files =
        matches.values_of_os("INPUT").unwrap();

    let buf_holder = BufferHolder::read_files(input_files)?;
    let file_holder = FileHolder::from_buffers(&buf_holder);

    let num_models = file_holder.models.len();
    let num_animations = file_holder.animations.len();

    let suf = |x| if x != 1 { "s" } else { "" };
    println!("Found {} model{}.", num_models, suf(num_models));
    println!("Found {} animation{}.", num_animations, suf(num_animations));

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

    let mut ui = Ui::new(&file_holder, &ctx)?;
    ui.run();

    Ok(())
}
