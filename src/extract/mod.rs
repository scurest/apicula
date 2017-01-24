use clap::ArgMatches;
use errors::Result;
use nitro::bca::Bca;
use nitro::bca::read_bca;
use nitro::bmd::Bmd;
use nitro::bmd::read_bmd;
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
    let mut num_bcas = 0;

    // Search for four bytes that match the stamp of a BMD or BCA file.
    // Then try to parse a file from that point. If we succeed, write
    // the bytes for that file to a new file in the output directory.
    let regex = Regex::new("(BMD0)|(BCA0)").unwrap();
    let mut cur_slice = &bytes[..];
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

                cur_slice = &cur_slice[file_end..];
            }
            Err(_) => {
                // There was an error parsing the Nitro file:
                // assume that means the four character matched
                // spriously, skip them, and go on.
                cur_slice = &cur_slice[found.start() + 4..];
            },
        }
    }

    let suf = |x| if x != 1 { "s" } else { "" };
    println!("Found {} BMD{}.", num_bmds, suf(num_bmds));
    println!("Found {} BCA{}.", num_bcas, suf(num_bcas));

    Ok(())
}

enum NitroContainer<'a> {
    Bmd(Bmd<'a>),
    Bca(Bca<'a>),
}

impl<'a> NitroContainer<'a> {
    fn file_size(&self) -> u32 {
        match *self {
            NitroContainer::Bmd(ref bmd) => bmd.file_size,
            NitroContainer::Bca(ref bca) => bca.file_size,
        }
    }
}

fn read_nitro_container(buf: &[u8]) -> Result<NitroContainer> {
    let cur = Cur::new(buf);
    let stamp = cur.clone().next_n_u8s(4)?;
    match stamp {
        b"BMD0" => Ok(NitroContainer::Bmd(read_bmd(cur)?)),
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
        NitroContainer::Bca(_) => "nsbca",
    }
}
