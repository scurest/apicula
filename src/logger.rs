//! Logger that prints messages like `[WARN] Lorem ipsum`.

use log::{self, Log, Level, Metadata, Record};

struct Logger {
    level: Level,
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            eprintln!("[{}] {}",
                record.level().to_string(),
                record.args(),
            );
        }
    }

    fn flush(&self) { }
}

pub fn init(level: Level) {
    let logger = Logger { level };
    let _ = log::set_boxed_logger(Box::new(logger));
    log::set_max_level(level.to_level_filter());
}
