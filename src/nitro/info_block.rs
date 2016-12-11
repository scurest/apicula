use errors::Result;
use std::fmt::Debug;
use std::iter::Zip;
use util::cur::Cur;
use util::name::Name;
use util::view::View;
use util::view::Viewable;

pub type Iterator<'a, T> = Zip<View<'a, T>, View<'a, Name>>;

/// Returns an iterator over the offset/name pairs in an info block.
pub fn read<T>(cur: Cur) -> Result<Iterator<T>> where
    T: Viewable + Debug
{
    fields!(cur, info_block {
        dummy: u8,
        count: u8,
        header_size: u16,

        unknown_subheader_size: u16,
        unknown_section_size: u16,
        unknown_constant: u32,
        unknown_data: [u32; count],

        size_of_datum: u16,
        data_section_size: u16,
        data: [T; count],

        names: [Name; count],
    });

    check!(dummy == 0);
    check!(size_of_datum as usize == <T as Viewable>::size());

    Ok(data.zip(names))
}
