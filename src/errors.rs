error_chain! {
    foreign_links {
        Fmt(::std::fmt::Error);
        Io(::std::io::Error);
        TimeFmt(::time::ParseError);
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
