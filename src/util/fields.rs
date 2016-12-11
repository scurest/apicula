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
