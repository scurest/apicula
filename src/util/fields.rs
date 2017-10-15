//! Macro for reading struct-like binary data.
//!
//! Reads a sequence of adjacent fields from a `Cur`, as though by `cur.next::<T>()?`.
//! Also logs the values read and their locations with `trace!` for debugging.
//!
//! # Examples
//!
//! ```
//! fields!(cur, MyName { // "MyName" is a human-readable name used only for logging
//!     // Read a u32, placing its value in the variable x.
//!     x: u32,
//!     // Read a u32 just after the end of the last one.
//!     y: u32,
//!     // Fields can depend on previous variables; read x u8s. z will be
//!     // a byte slice, &[u8].
//!     z: [u8; x],
//!     // This doesn't read anything, but stores a pointer to the current
//!     // location to c.
//!     c: Cur,
//!     // Read a 16-bit fixed-point number in (1,3,12) format. The extra
//!     // outer brackets are needed for syntactic reasons.
//!     f: (fix16(1,3,12)),
//! });
//! // If control reaches here, `cur` now points just past the end of the last field.
//! ```

macro_rules! field_helper2 {
    ($cur:ident, [u8; $n:expr]) => { $cur.next_n_u8s($n as usize)? };
    ($cur:ident, [$t:ty; $n:expr]) => { $cur.next_n::<$t>($n as usize)? };
    ($cur:ident, (fix16($s:expr,$i:expr,$f:expr))) => {
        {
            let x = $cur.next::<u16>()?;
            ::util::fixed::fix16(x, $s, $i, $f)
        }
    };
    ($cur:ident, (fix32($s:expr,$i:expr,$f:expr))) => {
        {
            let x = $cur.next::<u32>()?;
            ::util::fixed::fix32(x, $s, $i, $f)
        }
    };
    ($cur:ident, Cur) => { $cur.clone() };
    ($cur:ident, $t:ty) => { $cur.next::<$t>()? };
}

macro_rules! field_helper {
    ($c:ident, $name:ident, $field:ident, Cur) => {
        let $field = field_helper2!($c, Cur);
    };
    ($c:ident, $name:ident, $field:ident, $ty:tt) => {
        let pos = $c.pos();
        let $field = field_helper2!($c, $ty);
        trace!("{}.{}@{:#x}: {:?}",
            stringify!($name),
            stringify!($field),
            pos,
            $field,
        );
    }
}

macro_rules! fields {
    ($cur:expr, $name:ident { $($field:ident : $ty:tt,)* }) => {
        let mut c = $cur;
        $(field_helper!(c, $name, $field, $ty);)*
    };
    ($cur:ident, $name:ident { $($field:ident : $ty:tt),* }) => {
        fields!($cur, $name { $($field : $ty,)* });
    };
}
