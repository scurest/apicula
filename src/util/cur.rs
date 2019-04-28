use std::{fmt, error};
use std::ops::Add;
use util::view::{View, Viewable};

/// A pointer into a buffer of bytes. Used for binary file parsing.
#[derive(Copy, Clone)]
pub struct Cur<'a> {
    buf_: &'a [u8],
    pos_: usize,
}

impl<'a> Cur<'a> {
    pub fn new(buf: &[u8]) -> Cur {
        Cur { buf_: buf, pos_: 0 }
    }

    pub fn from_buf_pos(buf: &[u8], pos: usize) -> Cur {
        Cur { buf_: buf, pos_: pos }
    }

    pub fn pos(&self) -> usize {
        self.pos_
    }

    pub fn bytes_remaining(&self) -> usize {
        self.buf_.len().saturating_sub(self.pos_)
    }

    pub fn peek<T: Viewable>(&self) -> Result<T, Error> {
        let size = <T as Viewable>::size();
        if self.bytes_remaining() < size {
            return Err(Error::TooShort);
        }
        Ok(<T as Viewable>::view(&self.buf_[self.pos_..self.pos_ + size]))
    }

    pub fn next<T: Viewable>(&mut self) -> Result<T, Error> {
        let size = <T as Viewable>::size();
        if self.bytes_remaining() < size {
            return Err(Error::TooShort);
        }
        let next = <T as Viewable>::view(&self.buf_[self.pos_..self.pos_ + size]);
        self.pos_ += size;
        Ok(next)
    }

    pub fn nth<T: Viewable>(&self, n: usize) -> Result<T, Error> {
        Ok(self.clone().next_n::<T>(n+1)?.nth(n))
    }

    pub fn next_n<T: Viewable>(&mut self, n: usize) -> Result<View<'a, T>, Error> {
        let size = <T as Viewable>::size();
        let buf = self.next_n_u8s(size * n)?;
        Ok(View::from_buf(buf))
    }

    pub fn next_n_u8s(&mut self, n: usize) -> Result<&'a [u8], Error> {
        if self.pos_.saturating_add(n) > self.buf_.len() {
            return Err(Error::TooShort);
        }
        let next_n = &self.buf_[self.pos_..self.pos_ + n];
        self.pos_ += n;
        Ok(next_n)
    }

    pub fn slice_from_cur_to_end(&self) -> &'a [u8] {
        &self.buf_[self.pos_ ..]
    }

    pub fn jump_forward(&mut self, amt: usize) {
        let pos = self.pos_;
        self.jump_to(pos + amt);
    }

    pub fn jump_to(&mut self, pos: usize) {
        self.pos_ = pos;
    }
}

impl<'a> Add<usize> for Cur<'a> {
    type Output = Cur<'a>;

    fn add(self, amt: usize) -> Cur<'a> {
        let mut cur = self;
        cur.jump_forward(amt);
        cur
    }
}

impl<'a> Add<u32> for Cur<'a> {
    type Output = Cur<'a>;
    fn add(self, amt: u32) -> Cur<'a> { self + amt as usize }
}

impl<'a> Add<u16> for Cur<'a> {
    type Output = Cur<'a>;
    fn add(self, amt: u16) -> Cur<'a> { self + amt as usize }
}

impl<'a> fmt::Debug for Cur<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Cur {{ pos: {} }}", self.pos())
    }
}

#[derive(Debug)]
pub enum Error {
    TooShort,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "buffer too short")
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::TooShort => "buffer too short",
        }
    }
}
