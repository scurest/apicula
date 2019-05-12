//! Extract recognized container files from ROMs or other packed files.

use clap::ArgMatches;
use decompress;
use errors::Result;
use nitro::container::read_container;
use nitro::Container;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use util::cur::Cur;
use util::OutDir;

pub fn main(matches: &ArgMatches) -> Result<()> {
    let input_file = matches.value_of_os("INPUT").unwrap();
    let input = fs::read(&input_file)?;
    let cur = Cur::new(&input[..]);

    let out_dir_path = PathBuf::from(matches.value_of("OUTPUT").unwrap());
    let out_dir = OutDir::make_ready(out_dir_path)?;
    let mut output = ExtractOutput::new(out_dir);

    scan_for_nitro_files(&mut output, cur);
    scan_for_compressed_nitro_files(&mut output, cur);

    output.print_report();

    Ok(())
}

fn scan_for_nitro_files(output: &mut ExtractOutput, mut cur: Cur) {
    while let Some(start_idx) = find_next_stamp(cur.slice_from_cur_to_end()) {
        cur.jump_forward(start_idx);
        scan_for_file_at(output, &mut cur);
    }
}

/// Scans for a Nitro file at cur and outputs it if it exists.
/// Also moves cur to where you should resume searching from.
fn scan_for_file_at(output: &mut ExtractOutput, cur: &mut Cur) {
    if let Ok(cont) = read_container(*cur) {
        if let Ok(file_bytes) = cur.next_n_u8s(cont.file_size as usize) {
            output.save_file(file_bytes, &cont);
            return;
        }
    }
    cur.jump_forward(1);
}

fn scan_for_compressed_nitro_files(output: &mut ExtractOutput, mut cur: Cur) {
    while let Some(start_idx) = find_next_compression_start_byte(cur.slice_from_cur_to_end()) {
        cur.jump_forward(start_idx);
        if let Ok(result) = decompress::decompress(cur) {
            scan_for_nitro_files(output, Cur::new(&result.data));
        }
        cur.jump_forward(1);
    }
}

fn find_next_stamp(bytes: &[u8]) -> Option<usize> {
    // find BMD0|BTX0|BCA0|BTP0
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i] == b'B' && bytes[i + 3] == b'0' {
            if (bytes[i + 1] == b'M' && bytes[i + 2] == b'D')
                || (bytes[i + 1] == b'T' && bytes[i + 2] == b'X')
                || (bytes[i + 1] == b'C' && bytes[i + 2] == b'A')
                || (bytes[i + 1] == b'T' && bytes[i + 2] == b'P')
            {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn find_next_compression_start_byte(bytes: &[u8]) -> Option<usize> {
    // find 0x10|0x11
    let mut i = 0;
    while i != bytes.len() {
        if bytes[i] == 0x10 || bytes[i] == 0x11 {
            return Some(i);
        }
        i += 1;
    }
    None
}

struct ExtractOutput {
    taken_file_names: HashSet<String>,
    out_dir: OutDir,
    num_bmds: u32,
    num_btxs: u32,
    num_bcas: u32,
    num_btps: u32,
}

impl ExtractOutput {
    fn new(out_dir: OutDir) -> ExtractOutput {
        ExtractOutput {
            taken_file_names: HashSet::new(),
            out_dir,
            num_bmds: 0,
            num_btxs: 0,
            num_bcas: 0,
            num_btps: 0,
        }
    }

    /// Print report on extraction results.
    fn print_report(&self) {
        let plural = |x| if x != 1 { "s" } else { "" };
        println!(
            "Found {} BMD{}, {} BTX{}, {} BCA{}, {} BTP{}.",
            self.num_bmds,
            plural(self.num_bmds),
            self.num_btxs,
            plural(self.num_btxs),
            self.num_bcas,
            plural(self.num_bcas),
            self.num_btps,
            plural(self.num_btps),
        );
    }

    /// Given the slice `bytes` that successfully parsed as the Nitro container
    /// `container`, save the slice to a file in the output directory.
    fn save_file(&mut self, bytes: &[u8], container: &Container) {
        let file_name = guess_container_name(container);
        let file_extension = match container.stamp {
            b"BMD0" => "nsbmd",
            b"BTX0" => "nsbtx",
            b"BCA0" => "nsbca",
            b"BTP0" => "nsbtp",
            _ => "nsbxx",
        };

        // Find an available filename
        let mut save_path = format!("{}.{}", file_name, file_extension);
        let mut cntr = 1;
        while self.taken_file_names.contains(&save_path) {
            save_path.clear();
            use std::fmt::Write;
            write!(save_path, "{}.{:03}.{}", file_name, cntr, file_extension).unwrap();
            cntr += 1;
        }
        self.taken_file_names.insert(save_path.clone());

        let result = self
            .out_dir
            .create_file(&save_path)
            .and_then(|mut f| Ok(f.write_all(bytes)?));
        match result {
            Ok(()) => match container.stamp {
                b"BMD0" => self.num_bmds += 1,
                b"BTX0" => self.num_btxs += 1,
                b"BCA0" => self.num_bcas += 1,
                b"BTP0" => self.num_btps += 1,
                _ => (),
            },
            Err(e) => {
                error!("failed to write {}: {:?}", file_name, e);
            }
        }
    }
}

/// Guess a name for `cont` using the name of its first item.
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
            b"BMD0" => "empty_model_file",
            b"BTX0" => "empty_texture_file",
            b"BCA0" => "empty_animation_file",
            b"BTP0" => "empty_pattern_file",
            _ => "empty_unknown_file",
        }
        .to_string()
    }
}
