//! Handling for types that can be viewed as byte arrays.

use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Write;
use std::iter::Iterator;
use std::marker::PhantomData;

/// Types that can be constructed from a constant-length byte array.
pub trait Viewable: Sized {
    /// Size of the byte array.
    fn size() -> usize;
    /// View the byte-array (of length `size()`) as a `Self`.
    fn view(buf: &[u8]) -> Self;
}

impl Viewable for u8 {
    fn size() -> usize { 1 }
    fn view(buf: &[u8]) -> u8 { buf[0] }
}

impl Viewable for u16 {
    fn size() -> usize { 2 }
    fn view(buf: &[u8]) -> u16 {
        assert!(buf.len() >= 2);
        unsafe {
            use std::ptr::read_unaligned;
            let ptr = buf.as_ptr() as *const u8 as *const u16;
            read_unaligned(ptr).to_le()
        }
    }
}

impl Viewable for u32 {
    fn size() -> usize { 4 }
    fn view(buf: &[u8]) -> u32 {
        assert!(buf.len() >= 4);
        unsafe {
            use std::ptr::read_unaligned;
            let ptr = buf.as_ptr() as *const u8 as *const u32;
            read_unaligned(ptr).to_le()
        }
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

impl<T,S,P,Q,R> Viewable for (T,S,P,Q,R) where
    T: Viewable,
    S: Viewable,
    P: Viewable,
    Q: Viewable,
    R: Viewable,
{
    fn size() -> usize { <(T,S,(P,Q,R)) as Viewable>::size() }
    fn view(buf: &[u8]) -> (T,S,P,Q,R) {
        let (t,s,(p,q,r)) = <(T,S,(P,Q,R)) as Viewable>::view(buf);
        (t,s,p,q,r)
    }
}


/// A byte buffer interpreted as an array of Viewable elements.
#[derive(Copy, Clone)]
pub struct View<'a, T> {
    buf: &'a [u8],
    _marker: PhantomData<*const T>, // TODO: Is this the right type?
}

impl<'a, T: Viewable> View<'a, T> {
    /// Constructs a `View` from its underlying buffer.
    ///
    /// # Panics
    /// The view must contain an even number of `T`s; that is, the length
    /// of `buf` in bytes must be a multiple of the `Viewable::size()` of
    /// `T`.
    pub fn from_buf(buf: &[u8]) -> View<T> {
        let size = <T as Viewable>::size();
        assert!(size == 0 || buf.len() % size == 0);
        View { buf, _marker: PhantomData }
    }

    /// Number of `T`s in the view.
    pub fn len(&self) -> usize {
        let size = <T as Viewable>::size();
        self.buf.len() / size
    }

    pub fn get(&self, pos: usize) -> Option<T> {
        let size = <T as Viewable>::size();
        let begin = size * pos;
        let end = begin + size;
        if end > self.buf.len() {
            return None;
        }
        let bytes = &self.buf[begin..end];
        Some(<T as Viewable>::view(bytes))
    }

    pub fn nth(&self, pos: usize) -> T {
        match self.get(pos) {
            Some(x) => x,
            None => {
                panic!("index {} out of range for view of length {}",
                    pos, self.len());
            }
        }
    }
}

impl<'a, T: Viewable + Debug> Debug for View<'a, T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "View [")?;
        if self.len() != 0 {
            write!(f, "{:?}", self.nth(0))?;
            for i in 1..self.len() {
                write!(f, ", {:?}", self.nth(i))?;
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
