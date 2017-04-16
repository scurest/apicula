use std::process::Command;
use std::env;
use std::fs::File;
use std::path::Path;
use std::io::Write;

fn main() {
    // Write the current git hash to ${OUT_DIR}/git_rev
    // so it's available to main.rs

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("git_rev");
    let mut f = File::create(&dest_path).unwrap();

    let commit_hash =
        Command::new("git")
            .args(&["rev-parse", "HEAD"])
            .output().unwrap();
    let changes_in_working_dir =
        Command::new("git")
            .args(&["status", "--porcelain"])
            .output().unwrap();

    let was_error =
        !commit_hash.status.success() ||
        !changes_in_working_dir.status.success();

    if was_error {
        f.write_all(b"unknown revision").unwrap();
    } else {
        let wip = !changes_in_working_dir.stdout.is_empty();

        // Drop the trailing newline
        let hash = commit_hash.stdout.as_slice()
            .split_last().unwrap().1;

        if wip {
            f.write_all(b"WIP on ").unwrap();
        }
        f.write_all(hash).unwrap();
    }
}
