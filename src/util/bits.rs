//! Extracting bitfields from integers.
//!
//! If `x` is an unsigned integer, `x.bits(lo, hi)` is a number of
//! the same type with the bits of `x` in the range [`lo`, `hi`) in
//! its low part.
//!
//! # Examples
//! ```
//! let x = 0xabcdef00u32;
//! assert_eq!(x.bits(8, 16), 0xef);
//! assert_eq!(x.bits(16, 28), 0xbcd);
//! ```

pub trait BitField {
    fn bits(self, lo: u32, hi: u32) -> Self;
}

macro_rules! def_bitfield {
    ($t:ty, $bitwidth:expr) => {
        impl BitField for $t {
            #[inline(always)]
            fn bits(self, lo: u32, hi: u32) -> $t {
                assert!(lo <= hi);
                assert!(hi <= $bitwidth);
                (self >> lo) & (!0 >> ($bitwidth - (hi - lo) as $t))
            }
        }
    }
}

def_bitfield!(u8, 8);
def_bitfield!(u16, 16);
def_bitfield!(u32, 32);

#[test]
fn test() {
    let x = 0xabcdef00u32;
    assert_eq!(x.bits(8, 16), 0xef);
    assert_eq!(x.bits(16, 28), 0xbcd);
}
