use nitro::tex::Tex;
use nitro::tex::read_tex;
use util::cur::Cur;
use errors::Result;

#[derive(Debug, Clone)]
pub struct Btx<'a> {
    pub file_size: u32,
    pub texs: Vec<Tex<'a>>,
}

pub fn read_btx(cur: Cur) -> Result<Btx> {
    fields!(cur, BTX0 {
        stamp: [u8; 4],
        bom: u16,
        version: u16,
        file_size: u32,
        header_size: u16,
        num_sections: u16,
        section_offs: [u32; num_sections],
    });
    check!(stamp == b"BTX0")?;
    check!(bom == 0xfeff)?;
    check!(header_size == 16)?;
    check!(num_sections > 0)?;

    let texs = section_offs
        .map(|off| read_tex((cur + off as usize)?))
        .collect::<Result<_>>()?;

    Ok(Btx {
        file_size: file_size,
        texs: texs,
    })
}
