use std::fmt;
use std::fmt::Write;
use util::view::Viewable;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
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

/// Wrapper than prints a name as a non-empty sequence of
/// underscores, letters, and digits (safe for eg. filenames).
pub struct IdFmt<'a>(pub &'a Name);

impl<'a> fmt::Display for IdFmt<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let trimmed = trim_trailing_nul(&(self.0).0[..]);
        if trimmed.len() == 0 {
            f.write_char('_')?;
            return Ok(());
        }
        for &b in trimmed {
            let is_letter_or_digit =
                (b >= 'a' as u8 && b <= 'z' as u8) ||
                (b >= 'A' as u8 && b <= 'Z' as u8) ||
                (b >= '0' as u8 && b <= '9' as u8);
            let c = if is_letter_or_digit { b as char } else { '_' };
            f.write_char(c)?;
        }
        Ok(())
    }
}

fn trim_trailing_nul(mut buf: &[u8]) -> &[u8] {
    while let Some((&0, rest)) = buf.split_last() {
        buf = rest;
    }
    buf
}
