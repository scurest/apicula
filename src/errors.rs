use std::error::Error;
use std::fmt;

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Error message.
#[derive(Debug)]
pub struct ErrorMsg {
    pub msg: String,
}

impl fmt::Display for ErrorMsg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.msg)
    }
}

impl Error for ErrorMsg {}

macro_rules! errmsg {
    ($msg:expr) => {
        crate::errors::ErrorMsg { msg: $msg.into() }
    };
    ($fmt:expr, $($arg:tt)+) => {
        crate::errors::ErrorMsg { msg: format!($fmt, $($arg)+) }
    };
}

macro_rules! bail {
    ($($arg:tt)+) => {
        return Err(errmsg!($($arg)+).into())
    };
}

macro_rules! check {
    ($b:expr) => {
        if !$b {
            Err(errmsg!(concat!("expected: ", stringify!($b))))
        } else {
            Ok(())
        }
    };
}
