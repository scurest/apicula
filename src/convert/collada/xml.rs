//! Helpers to make writing XML less unpleasant.

use cgmath::Matrix4;
use std::fmt::{Display, Write};

pub struct Xml {
    s: String,
    cur_indent: u32,
}

static INDENT_SIZE: u32 = 2;

impl Xml {
    pub fn with_capacity(capacity: usize) -> Xml {
        Xml {
            s: String::with_capacity(capacity),
            cur_indent: 0,
        }
    }

    pub fn string(self) -> String {
        self.s
    }

    pub fn start_open_tag(&mut self) {
        self.s.push_str("<");
        self.cur_indent += 1;
    }

    pub fn start_close_tag(&mut self) {
        self.s.push_str("</");
        self.cur_indent -= 1;
    }

    pub fn deindent_and_start_close_tag(&mut self) {
        for _ in 0..INDENT_SIZE {
            self.s.pop();
        }
        self.s.push_str("</");
        self.cur_indent -= 1;
    }

    pub fn nl(&mut self) {
        self.s.push_str("\n");
        for _ in 0..self.cur_indent {
            for _ in 0..INDENT_SIZE {
                self.s.push(' ');
            }
        }
    }

    pub fn end_tag(&mut self) {
        self.s.push_str(">");
    }

    pub fn end_empty_tag(&mut self) {
        self.s.push_str("/>");
        self.cur_indent -= 1;
    }

    pub fn push_str(&mut self, s: &str) {
        self.s.push_str(s);
    }

    pub fn push_text<T: Display>(&mut self, x: &T) {
        write!(&mut self.s, "{}", x).unwrap();
    }

    pub fn matrix(&mut self, m: &Matrix4<f64>) {
        let m: &[f64; 16] = m.as_ref();
        // COLLADA wants row-major order
        write!(
            &mut self.s,
            "{} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
            m[0],
            m[4],
            m[8],
            m[12],
            m[1],
            m[5],
            m[9],
            m[13],
            m[2],
            m[6],
            m[10],
            m[14],
            m[3],
            m[7],
            m[11],
            m[15],
        )
        .unwrap();
    }
}

macro_rules! xml_attr {
    ($x:ident; ($e:expr) $($rest:tt)*) => {
        $x.push_text(&$e);
        xml_attr!($x; $($rest)*);
    };
    ($x:ident; $strlit:tt $($rest:tt)*) => {
        $x.push_str($strlit);
        xml_attr!($x; $($rest)*);
    };
    ($x:ident;) => {};
}

macro_rules! xml {
    ($x:ident; </ $($rest:tt)*) => {
        $x.start_close_tag();
        xml!($x; $($rest)*)
    };
    ($x:ident; < $($rest:tt)*) => {
        $x.start_open_tag();
        xml!($x; $($rest)*)
    };
    ($x:ident; /> $($rest:tt)*) => {
        $x.end_empty_tag();
        xml!($x; $($rest)*)
    };
    ($x:ident; > $($rest:tt)*) => {
        $x.end_tag();
        xml!($x; $($rest)*)
    };
    ($x:ident; ; $($rest:tt)*) => {
        $x.nl();
        xml!($x; $($rest)*)
    };
    ($x:ident; / $($rest:tt)*) => {
        $x.deindent_and_start_close_tag();
        xml!($x; $($rest)*)
    };
    ($x:ident; $attr:ident = [ $($val:tt)* ] $($rest:tt)*) => {
        $x.push_str(" ");
        $x.push_str(stringify!($attr));
        $x.push_str("=\"");
        xml_attr!($x; $($val)*);
        $x.push_str("\"");
        xml!($x; $($rest)*)
    };
    ($x:ident; if ($cond:expr) { $($then:tt)* } else { $($els:tt)* } $($rest:tt)*) => {
        if $cond {
            xml!($x; $($then)*);
        } else {
            xml!($x; $($els)*);
        }
        xml!($x; $($rest)*);
    };
    ($x:ident; if ($cond:expr) { $($then:tt)* } $($rest:tt)*) => {
        if $cond {
            xml!($x; $($then)*);
        }
        xml!($x; $($rest)*);
    };
    ($x:ident; for $p:pat in ($it:expr) { $($body:tt)* } $($rest:tt)*) => {
        for $p in $it {
            xml!($x; $($body)*);
        }
        xml!($x; $($rest)*);
    };
    ($x:ident; MATRIX($e:expr) $($rest:tt)*) => {
        $x.matrix($e);
        xml!($x; $($rest)*);
    };
    ($x:ident; $word:ident $($rest:tt)*) => {
        $x.push_str(stringify!($word));
        xml!($x; $($rest)*)
    };
    ($x:ident; ($e:expr) $($rest:tt)*) => {
        $x.push_text(&$e);
        xml!($x; $($rest)*)
    };
    ($x:ident; $strlit:tt $($rest:tt)*) => {
        $x.push_str(&$strlit);
        xml!($x; $($rest)*)
    };
    ($x:ident;) => {};
}
