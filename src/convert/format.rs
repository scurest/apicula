use cgmath::Matrix4;
use std::fmt;

macro_rules! cat_lines {
    ($s:expr) => { concat!($s, "\n") };
    ($s:expr, $($ss:expr),*) => { concat!($s, "\n", cat_lines!($($ss),*)) };
}

macro_rules! write_lines {
    ($dst:expr, $($fmt_strs:expr),*; $($args:tt)*) => {
        write!($dst, cat_lines!($($fmt_strs),*), $($args)*)
    };
}

pub struct FnFmt<F: Fn(&mut fmt::Formatter) -> fmt::Result>(pub F);

impl<F> fmt::Display for FnFmt<F>
where F: Fn(&mut fmt::Formatter) -> fmt::Result {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.0)(f)
    }
}

pub struct Mat<'a>(pub &'a Matrix4<f64>);

impl<'a> fmt::Display for Mat<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
            self.0.x.x, self.0.y.x, self.0.z.x, self.0.w.x,
            self.0.x.y, self.0.y.y, self.0.z.y, self.0.w.y,
            self.0.x.z, self.0.y.z, self.0.z.z, self.0.w.z,
            self.0.x.w, self.0.y.w, self.0.z.w, self.0.w.w,
        )
    }
}