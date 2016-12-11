use std::fmt;
use std::fmt::Write;
use util::view::Viewable;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Name(pub [u8; 16]);

impl Name {
    pub fn from_bytes(buf: &[u8]) -> Name {
        let mut arr = [0; 16];
        for (i, &b) in buf.iter().enumerate() {
            arr[i] = b;
        }
        Name(arr)
    }
}

impl Viewable for Name {
    fn size() -> usize { 16 }
    fn view(buf: &[u8]) -> Name {
        Name::from_bytes(buf)
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for &b in trim_trailing_nul(&self.0[..]) {
            f.write_char(if b < 0x20 { '.' } else { b as char })?;
        }
        Ok(())
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_char('"')?;
        for &b in trim_trailing_nul(&self.0[..]) {
            for c in (b as char).escape_default() {
                f.write_char(c)?;
            }
        }
        f.write_char('"')
    }
}

fn trim_trailing_nul(mut buf: &[u8]) -> &[u8] {
    while let Some((&0, rest)) = buf.split_last() {
        buf = rest;
    }
    buf
}
