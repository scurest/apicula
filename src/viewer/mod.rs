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
use db::Database;

pub fn main(matches: &ArgMatches) -> Result<()> {
    let db = Database::from_arg_matches(matches)?;

    db.print_status();

    if db.models.is_empty() {
        println!("No models. Nothing to do.\n");
        return Ok(())
    }

    let mut ui = Ui::new(db)?;
    ui.run();

    Ok(())
}
