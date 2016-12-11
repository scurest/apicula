use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Write;
use std::iter::Iterator;
use std::marker::PhantomData;

/// Types that can be viewed as a fixed-length byte sequence.
pub trait Viewable: Sized {
    fn size() -> usize;
    fn view(buf: &[u8]) -> Self;
}

impl Viewable for u8 {
    fn size() -> usize { 1 }
    fn view(buf: &[u8]) -> u8 { buf[0] }
}

impl Viewable for u16 {
    fn size() -> usize { 2 }
    fn view(buf: &[u8]) -> u16 {
        buf[0] as u16 | (buf[1] as u16) << 8
    }
}

impl Viewable for u32 {
    fn size() -> usize { 4 }
    fn view(buf: &[u8]) -> u32 {
        buf[0] as u32 | (buf[1] as u32) << 8 | (buf[2] as u32) << 16 | (buf[3] as u32) << 24
    }
}

impl<T,S> Viewable for (T,S) where
    T: Viewable,
    S: Viewable
{
    fn size() -> usize { <T as Viewable>::size() + <S as Viewable>::size() }
    fn view(buf: &[u8]) -> (T,S) {
        let split = <T as Viewable>::size();
        let t = <T as Viewable>::view(&buf[..split]);
        let s = <S as Viewable>::view(&buf[split..]);
        (t,s)
    }
}

impl<T,S,P> Viewable for (T,S,P) where
    T: Viewable,
    S: Viewable,
    P: Viewable,
{
    fn size() -> usize { <(T,(S,P)) as Viewable>::size() }
    fn view(buf: &[u8]) -> (T,S,P) {
        let (t,(s,p)) = <(T,(S,P)) as Viewable>::view(buf);
        (t,s,p)
    }
}

/// An byte buffer interpreted as an array of Viewable elements.
#[derive(Copy, Clone)]
pub struct View<'a, T> {
    buf: &'a [u8],
    _marker: PhantomData<*const T>, // TODO: Is this the right type?
}

impl<'a, T: Viewable> View<'a, T> {
    pub fn from_buf(buf: &[u8]) -> View<T> {
        let size = <T as Viewable>::size();
        assert!(size == 0 || buf.len() % size == 0);
        View { buf: buf, _marker: PhantomData }
    }

    pub fn len(&self) -> usize {
        let size = <T as Viewable>::size();
        self.buf.len() / size
    }

    pub fn get(&self, pos: usize) -> T {
        let size = <T as Viewable>::size();
        let begin = size * pos;
        let end = begin + size;
        if end > self.buf.len() {
            panic!("index {} out of range for view of length {}", pos, self.buf.len());
        }
        let bytes = &self.buf[begin..end];
        <T as Viewable>::view(bytes)
    }
}

impl<'a, T: Viewable + Debug> Debug for View<'a, T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "View [")?;
        if self.len() != 0 {
            write!(f, "{:?}", self.get(0))?;
            for i in 1..self.len() {
                write!(f, ", {:?}", self.get(i))?;
            }
        }
        f.write_char(']')
    }
}

impl<'a, T: Viewable> Iterator for View<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.buf.len() == 0 {
            None
        } else {
            let size = <T as Viewable>::size();
            let item = <T as Viewable>::view(&self.buf[0..size]);
            self.buf = &self.buf[size..];
            Some(item)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, T: Viewable> DoubleEndedIterator for View<'a, T> {
    fn next_back(&mut self) -> Option<T> {
        if self.buf.len() == 0 {
            None
        } else {
            let size = <T as Viewable>::size();
            let idx = self.buf.len() - size;
            let item = <T as Viewable>::view(&self.buf[idx..]);
            self.buf = &self.buf[..idx];
            Some(item)
        }
    }
}

impl<'a, T: Viewable> ExactSizeIterator for View<'a, T> {}
