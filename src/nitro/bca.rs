use nitro::jnt::Jnt;
use nitro::jnt::read_jnt;
use util::cur::Cur;
use errors::Result;

#[derive(Debug, Clone)]
pub struct Bca<'a> {
    pub file_size: u32,
    pub jnts: Vec<Jnt<'a>>,
}

pub fn read_bca(cur: Cur) -> Result<Bca> {
    fields!(cur, BCA0 {
        stamp: [u8; 4],
        bom: u16,
        version: u16,
        file_size: u32,
        header_size: u16,
        num_sections: u16,
        section_offs: [u32; num_sections],
    });
    check!(stamp == b"BCA0");
    check!(bom == 0xfeff);
    check!(header_size == 16);
    check!(num_sections > 0);

    let jnts = section_offs
        .map(|off| read_jnt((cur + off as usize)?))
        .collect::<Result<_>>()?;

    Ok(Bca {
        file_size: file_size,
        jnts: jnts,
    })
}
