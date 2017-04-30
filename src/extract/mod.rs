use clap::ArgMatches;
use errors::Result;
use nitro::bca::Bca;
use nitro::bca::read_bca;
use nitro::bmd::Bmd;
use nitro::bmd::read_bmd;
use nitro::btx::Btx;
use nitro::btx::read_btx;
use nitro::name::IdFmt;
use regex::bytes::Regex;
use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use util::cur::Cur;
use util::uniq::UniqueNamer;

pub fn main(matches: &ArgMatches) -> Result<()> {
    let input_file = matches
        .value_of_os("INPUT").unwrap();
    let mut f = fs::File::open(input_file)?;
    let mut bytes = vec![];
    f.read_to_end(&mut bytes)?;

    let out_dir = PathBuf::from(matches.value_of("OUTPUT").unwrap());
    fs::create_dir(&out_dir)?;

    let mut file_namer = UniqueNamer::new();

    let mut num_bmds = 0;
    let mut num_btxs = 0;
    let mut num_bcas = 0;

    // Search for four bytes that match the stamp of a BMD, BTX, or BCA
    // file. Then try to parse a file from that point. If we succeed, write
    // the bytes for that file to a new file in the output directory.

    let regex = Regex::new("(BMD0)|(BTX0)|(BCA0)").unwrap();

    // The suffix of bytes that we have yet to search.
    // Invariant: cur_slice = &bytes[cur_pos..]
    let mut cur_slice = &bytes[..];
    let mut cur_pos = 0;

    while let Some(found) = regex.find(cur_slice) {
        let res = read_nitro_container(&cur_slice[found.start()..]);
        match res {
            Ok(cont) => {
                let file_end = found.start() + cont.file_size() as usize;
                let file_slice = &cur_slice[found.start()..file_end];

                let name = file_namer.get_name(guess_name(&cont));
                let ext = extension(&cont);
                let out_path = out_dir.join(&format!("{}.{}", name, ext));

                let res = create_and_write_file(&out_path, file_slice);
                match res {
                    Ok(()) => {
                        match cont {
                            NitroContainer::Bmd(_) => num_bmds += 1,
                            NitroContainer::Btx(_) => num_btxs += 1,
                            NitroContainer::Bca(_) => num_bcas += 1,
                        }
                    }
                    Err(e) => {
                        error!("failed to write {}: {:?}",
                            out_path.to_string_lossy(),
                            e,
                        );
                    }
                }

                cur_pos += file_end;
                cur_slice = &cur_slice[file_end..];
            }
            Err(e) => {
                // There was an error parsing the Nitro file:
                // assume that means the four character matched
                // spuriously, skip them, and go on.
                cur_pos += found.start() + 4;
                cur_slice = &cur_slice[found.start() + 4..];

                debug!("tried to parse a file with stamp {:?} at offset {}, but failed: \
                    the error was: {}",
                    found.as_bytes(), cur_pos, e,
                );
            },
        }
    }

    let suf = |x| if x != 1 { "s" } else { "" };
    println!("Found {} BMD{}.", num_bmds, suf(num_bmds));
    println!("Found {} BTX{}.", num_btxs, suf(num_btxs)); // er, maybes BTXes?
    println!("Found {} BCA{}.", num_bcas, suf(num_bcas));

    Ok(())
}

enum NitroContainer<'a> {
    Bmd(Bmd<'a>),
    Btx(Btx<'a>),
    Bca(Bca<'a>),
}

impl<'a> NitroContainer<'a> {
    fn file_size(&self) -> u32 {
        match *self {
            NitroContainer::Bmd(ref bmd) => bmd.file_size,
            NitroContainer::Btx(ref btx) => btx.file_size,
            NitroContainer::Bca(ref bca) => bca.file_size,
        }
    }
}

fn read_nitro_container(buf: &[u8]) -> Result<NitroContainer> {
    let cur = Cur::new(buf);
    let stamp = cur.clone().next_n_u8s(4)?;
    match stamp {
        b"BMD0" => Ok(NitroContainer::Bmd(read_bmd(cur)?)),
        b"BTX0" => Ok(NitroContainer::Btx(read_btx(cur)?)),
        b"BCA0" => Ok(NitroContainer::Bca(read_bca(cur)?)),
        _ => Err("not a container".into())
    }
}

fn create_and_write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut f = fs::File::create(&path)?;
    f.write_all(bytes)?;
    Ok(())
}

/// Guess a name for the container `cont`, using the name of the first
/// item it contains.
fn guess_name(cont: &NitroContainer) -> String {
    match *cont {
        NitroContainer::Bmd(ref bmd) => {
            bmd.mdls.get(0)
                .and_then(|mdl| mdl.models.get(0))
                .map(|model| format!("{}", IdFmt(&model.name)))
                .unwrap_or_else(|| "BMD".to_string())
        }
        NitroContainer::Btx(ref btx) => {
            btx.texs.get(0)
                .and_then(|tex| tex.texinfo.get(0))
                .map(|texinfo| format!("{}", IdFmt(&texinfo.name)))
                .unwrap_or_else(|| "BTX".to_string())
        }
        NitroContainer::Bca(ref bca) => {
            bca.jnts.get(0)
                .and_then(|jnt| jnt.animations.get(0))
                .map(|anim| format!("{}", IdFmt(&anim.name)))
                .unwrap_or_else(|| "BCA".to_string())
        }
    }
}

fn extension(cont: &NitroContainer) -> &'static str {
    match *cont {
        NitroContainer::Bmd(_) => "nsbmd",
        NitroContainer::Btx(_) => "nsbtx",
        NitroContainer::Bca(_) => "nsbca",
    }
}
