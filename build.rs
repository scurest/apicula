extern crate time;

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;

fn main() {
    write_git_rev();
    write_compile_date();
}

/// Write the current git hash to ${OUT_DIR}/git-commit
/// so it's available to main.rs
fn write_git_rev() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dst_path = Path::new(&out_dir).join("git-commit");
    let mut f = File::create(&dst_path).unwrap();

    let commit_hash = Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
        .unwrap();
    let changes_in_working_dir = Command::new("git")
        .args(&["status", "--porcelain"])
        .output()
        .unwrap();

    let was_error = !commit_hash.status.success() || !changes_in_working_dir.status.success();

    if was_error {
        f.write_all(b"unknown commit").unwrap();
    } else {
        let wip = !changes_in_working_dir.stdout.is_empty();

        // Drop the trailing newline
        let hash = commit_hash.stdout.as_slice().split_last().unwrap().1;

        if wip {
            f.write_all(b"WIP ").unwrap();
        }
        f.write_all(hash).unwrap();
    }
}

fn write_compile_date() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dst_path = Path::new(&out_dir).join("compile-date");
    let mut f = File::create(&dst_path).unwrap();

    let now = time::now_utc();
    let date = time::strftime("%Y-%m-%d", &now).unwrap();

    f.write_all(date.as_bytes()).unwrap();
}
