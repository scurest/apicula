error_chain! {}

macro_rules! check {
    ($b:expr) => {
        if !$b {
            error!("expected: {})", stringify!($b));
            return Err("sanity check failed".into());
        }
    };
}
