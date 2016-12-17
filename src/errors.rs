error_chain! {
    foreign_links {
        Fmt(::std::fmt::Error);
        Io(::std::io::Error);
        TimeFmt(::time::ParseError);
        Png(::pnglib::EncodingError);
    }
}

macro_rules! check {
    ($b:expr) => {
        if !$b {
            error!("expected: {})", stringify!($b));
            return Err("sanity check failed".into());
        }
    };
}
