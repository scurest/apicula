use errors::Result;
use std::fmt;
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

    pub fn pos(&self) -> usize {
        self.pos_
    }

    pub fn bytes_remaining(&self) -> usize {
        self.buf_.len().saturating_sub(self.pos_)
    }

    pub fn next<T: Viewable>(&mut self) -> Result<T> {
        Ok(self.next_n::<T>(1)?.nth(0))
    }

    pub fn nth<T: Viewable>(&self, n: usize) -> Result<T> {
        Ok(self.clone().next_n::<T>(n+1)?.nth(n))
    }

    pub fn next_n<T: Viewable>(&mut self, n: usize) -> Result<View<'a, T>> {
        let size = <T as Viewable>::size();
        let buf = self.next_n_u8s(size * n)?;
        Ok(View::from_buf(buf))
    }

    pub fn next_n_u8s(&mut self, n: usize) -> Result<&'a [u8]> {
        let end_pos = self.pos_ + n;
        if end_pos > self.buf_.len() {
            bail!("buffer was too short");
        }
        let res = &self.buf_[self.pos_ .. self.pos_ + n];
        self.pos_ += n;
        Ok(res)
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

