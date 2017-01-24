use nitro::mdl::Mdl;
use nitro::mdl::read_mdl;
use nitro::tex::Tex;
use nitro::tex::read_tex;
use util::cur::Cur;
use errors::Result;

#[derive(Debug, Clone)]
pub struct Bmd<'a> {
    pub file_size: u32,
    pub mdls: Vec<Mdl<'a>>,
    pub texs: Vec<Tex<'a>>,
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

    let mut mdls = vec![];
    let mut texs = vec![];

    for section_off in section_offs {
        let section_cur = (cur + section_off as usize)?;
        let stamp = section_cur.clone().next_n_u8s(4)?;
        match stamp {
            b"MDL0" => mdls.push(read_mdl(section_cur)?),
            b"TEX0" => texs.push(read_tex(section_cur)?),
            _ => {
                info!("section with unknown stamp {:?} in BMD", stamp);
            }
        }
    }

    Ok(Bmd {
        file_size: file_size,
        mdls: mdls,
        texs: texs,
    })
}
