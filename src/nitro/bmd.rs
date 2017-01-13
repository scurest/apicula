use nitro::mdl::Mdl;
use nitro::mdl::read_mdl;
use nitro::tex::Tex;
use nitro::tex::read_tex;
use util::cur::Cur;
use errors::Result;

#[derive(Debug, Clone)]
pub struct Bmd<'a> {
    pub mdl: Mdl<'a>,
    pub tex: Tex<'a>,
}

pub fn read_bmd(cur: Cur) -> Result<Bmd> {
    fields!(cur, BMD0 {
        stamp: [u8; 4],
        bom: u16,
        version: u16,
        file_size: u32,
        header_size: u16,
        num_sections: u16,
        section_offs: [u32; num_sections],
    });
    check!(stamp == b"BMD0");
    check!(bom == 0xfeff);
    check!(header_size == 16);
    check!(num_sections > 0);

    let mdl_cur = (cur + section_offs.get(0) as usize)?;
    let tex_cur = (cur + section_offs.get(1) as usize)?;

    let mdl = read_mdl(mdl_cur)?;
    let tex = read_tex(tex_cur)?;

    Ok(Bmd {
        mdl: mdl,
        tex: tex,
    })
}
