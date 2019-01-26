//! Extract recognized container files from ROMs or other packed files.

use clap::ArgMatches;
use decompress::try_decompress;
use errors::Result;
use nitro::Container;
use nitro::container::read_container;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use util::cur::Cur;
use util::namers::UniqueNamer;
use util::OutDir;

pub fn main(matches: &ArgMatches) -> Result<()> {
    let input_file = matches.value_of_os("INPUT").unwrap();
    let input = fs::read(&input_file)?;

    let out_dir_path = PathBuf::from(matches.value_of("OUTPUT").unwrap());
    let out_dir = OutDir::make_ready(out_dir_path)?;

    let mut extractor = Extractor::new(out_dir);

    // Search for four bytes that match the stamp of a BMD, BTX, or BCA
    // file. Then try to parse a file from that point. If we succeed, write
    // the bytes for that file to a new file in the output directory.
    let mut cur = Cur::new(&input[..]);

    while let Some(start_idx) = find_next_stamp(cur.slice_from_cur_to_end()) {
        cur.jump_forward(start_idx);
        extractor.try_proc_file_at(&mut cur);
    }

    extractor.print_report();

    Ok(())
}

struct Extractor {
    out_dir: OutDir,
    /// Assigns unique names to the found files, so their
    /// file names in the save directory won't collide.
    file_namer: UniqueNamer,
    num_bmds: u32,
    num_btxs: u32,
    num_bcas: u32,
    num_btps: u32,
}

impl Extractor {
    fn new(out_dir: OutDir) -> Extractor {
        Extractor {
            out_dir,
            file_namer: UniqueNamer::new(),
            num_bmds: 0,
            num_btxs: 0,
            num_bcas: 0,
            num_btps: 0,
        }
    }

    /// Print a report on how extraction went; namely, the number of each kind of
    /// file found.
    fn print_report(&self) {
        let plural = |x| if x != 1 { "s" } else { "" };
        println!("Found {} BMD{}, {} BTX{}, {} BCA{}, {} BTP{}.",
            self.num_bmds, plural(self.num_bmds),
            self.num_btxs, plural(self.num_btxs),
            self.num_bcas, plural(self.num_bcas),
            self.num_btps, plural(self.num_btps),
        );
    }

    /// Assuming a Nitro stamp is found at `cur`, try to detect a container
    /// file there (either raw or compressed) and if successful, write the
    /// bytes to a file in the output directory.
    ///
    /// Afterwards, `cur` is positioned where you should resume searching (ie.
    /// after the container file if found, or else after the stamp if not.)
    fn try_proc_file_at(&mut self, cur: &mut Cur) {
        if let Ok(cont) = read_container(*cur) {
            if let Ok(file_bytes) = cur.next_n_u8s(cont.file_size as usize) {
                self.save_file(file_bytes, &cont);
                return;
            }
        }

        self.try_proc_compressed_file_at(cur)
    }

    /// Try decompressing data near `cur` and then attempt to parse a
    /// Nitro container from the decompressed data. If successful, write
    /// the decompressed data to a file in the save directory.
    ///
    /// Afterwards, `cur` is positioned where you should resume searching (ie.
    /// after the compressed stream if found, or else after the stamp if not.)
    fn try_proc_compressed_file_at(&mut self, cur: &mut Cur) {
        let res = try_decompress(*cur);
        match res {
            Ok(dec_res) => {
                let buf = &dec_res.data[..];
                let res = read_container(Cur::new(buf));
                match res {
                    Ok(cont) => {
                        self.save_file(buf, &cont);
                        *cur = dec_res.end_cur;
                    }
                    Err(_) => {
                        cur.jump_forward(4);
                    }
                }
            }
            Err(_) => {
                cur.jump_forward(4);
            }
        }
    }

    /// Given the slice `bytes` that successfully parsed as the Nitro container
    /// `container`, save the slice to a file in the save directory.
    fn save_file(&mut self, bytes: &[u8], container: &Container) {
        let file_name = self.file_namer.get_fresh_name(guess_container_name(container));
        let file_extension = match container.stamp {
            b"BMD0" => "nsbmd",
            b"BTX0" => "nsbtx",
            b"BCA0" => "nsbca",
            b"BTP0" => "nsbtp",
            _ => "nsbxx",
        };

        let file_name = format!("{}.{}", file_name, file_extension);
        let res =
            self.out_dir.create_file(&file_name)
            .and_then(|mut f| Ok(f.write_all(bytes)?));
        match res {
            Ok(()) => {
                match container.stamp {
                    b"BMD0" => self.num_bmds += 1,
                    b"BTX0" => self.num_btxs += 1,
                    b"BCA0" => self.num_bcas += 1,
                    b"BTP0" => self.num_btps += 1,
                    _ => (),
                }
            }
            Err(e) => {
                error!("failed to write {}: {:?}", file_name, e);
            }
        }
    }
}

/// Guess a name for the container `cont`, using the name of the first
/// item it contains.
fn guess_container_name(cont: &Container) -> String {
    if !cont.models.is_empty() {
        format!("{}", cont.models[0].name.print_safe())
    } else if !cont.textures.is_empty() {
        format!("{}", cont.textures[0].name.print_safe())
    } else if !cont.palettes.is_empty() {
        format!("{}", cont.palettes[0].name.print_safe())
    } else if !cont.animations.is_empty() {
        format!("{}", cont.animations[0].name.print_safe())
    } else if !cont.patterns.is_empty() {
        format!("{}", cont.patterns[0].name.print_safe())
    } else {
        match cont.stamp {
            b"BMD0" => "model_file",
            b"BTX0" => "texture_file",
            b"BCA0" => "animation_file",
            b"BTP0" => "pattern_file",
            _ => "unknown_file",
        }.to_string()
    }
}

pub fn find_next_stamp(bytes: &[u8]) -> Option<usize> {
    // find BMD0|BTX0|BCA0|BTP0
    let mut i = 0;
    while i < bytes.len() - 3 {
        if bytes[i] == b'B' && bytes[i+3] == b'0' {
            if (bytes[i+1] == b'M' && bytes[i+2] == b'D') ||
               (bytes[i+1] == b'T' && bytes[i+2] == b'X') ||
               (bytes[i+1] == b'C' && bytes[i+2] == b'A') ||
               (bytes[i+1] == b'T' && bytes[i+2] == b'P') {
                return Some(i);
            }
        }
        i += 1;
    }
    return None;
}
