//! Logger that prints messages like `[WARN] Lorem ipsum`.

use atty;
use log::{self, Log, Level, Metadata, Record};
use std::io::Write;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

struct Logger {
    level: Level,
    use_color: bool,
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let color_choice = match self.use_color {
            true => ColorChoice::Auto,
            false => ColorChoice::Never,
        };
        let mut stderr = StandardStream::stderr(color_choice);
        let _ = stderr.set_color(ColorSpec::new().set_fg(Some(Color::Green)));
        let _ = writeln!(&mut stderr, "[{}] {}",
            record.level().to_string(),
            record.args(),
        );
        let _ = stderr.reset();
    }

    fn flush(&self) { }
}

pub fn init(level: Level) {
    let use_color = atty::is(atty::Stream::Stderr);
    let logger = Logger { level, use_color };
    let _ = log::set_boxed_logger(Box::new(logger));
    log::set_max_level(level.to_level_filter());
}
