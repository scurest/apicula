use errors::Result;
use nitro::jnt::Jnt;
use nitro::jnt::read_jnt;
use nitro::mdl::Mdl;
use nitro::mdl::read_mdl;
use nitro::tex::read_tex;
use nitro::tex::Tex;
use util::cur::Cur;

/// Represents a Nitro container.
///
/// One data type is used for all the different containers (NSBMD,
/// NSBTX, NSBCA). Practically a NSBMD (say) should only contain
/// MDLs and TEXs, not JNTs. But there's no reason to treat any of
/// them any differently, so we permit each one to contain any
/// kind of data.
#[derive(Debug)]
pub struct Container<'a> {
    pub stamp: &'static [u8],
    pub file_size: u32,
    pub data_files: Vec<Result<DataFile<'a>>>,
}

/// One of the stamps for a known Nitro file.
static STAMPS: [&[u8]; 3] = [b"BMD0", b"BTX0", b"BCA0"];

impl<'a> Container<'a> {
    pub fn read(cur: Cur<'a>) -> Result<Container<'a>> {
        fields!(cur, BMD0 {
            stamp: [u8; 4],
            bom: u16,
            version: u16,
            file_size: u32,
            header_size: u16,
            num_sections: u16,
            section_offs: [u32; num_sections],
        });

        let stamp =
            match STAMPS.iter().find(|&s| s == &stamp) {
                Some(x) => x,
                None => bail!("unrecognized Nitro container: expected \
                    the first four bytes to be one of: BMD0, BTX0, BCA0"),
            };

        check!(bom == 0xfeff)?;
        check!(header_size == 16)?;
        check!(num_sections > 0)?;

        let data_files = section_offs
            .map(|off| {
                let res = DataFile::read((cur + off as usize)?);
                if let Err(ref e) = res {
                    info!("error reading section: {}", e);
                }
                res
            })
            .collect();

        Ok(Container { stamp, file_size, data_files })
    }
}

#[derive(Debug, Clone)]
pub enum DataFile<'a> {
    Mdl(Mdl<'a>),
    Tex(Tex<'a>),
    Jnt(Jnt<'a>),
}

impl<'a> DataFile<'a> {
    pub fn read(cur: Cur<'a>) -> Result<DataFile<'a>> {
        let stamp = cur.clone().next_n_u8s(4)?;
        Ok(match stamp {
            b"MDL0" => DataFile::Mdl(read_mdl(cur)?),
            b"TEX0" => DataFile::Tex(read_tex(cur)?),
            b"JNT0" => DataFile::Jnt(read_jnt(cur)?),
            _ => bail!("unrecognized Nitro format: expected the first four \
                bytes to be one of: MDL0, TEX0, JNT0"),
        })
    }
}
