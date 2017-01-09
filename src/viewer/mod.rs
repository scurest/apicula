mod viewer;

use clap::ArgMatches;
use errors::Result;
use files::BufferHolder;
use files::FileHolder;

pub fn main(matches: &ArgMatches) -> Result<()> {
    let input_files = matches
        .values_of_os("INPUT").unwrap();
    let buf_holder = BufferHolder::read_files(input_files)?;
    let file_holder = FileHolder::from_buffers(&buf_holder);

    viewer::viewer(file_holder)?;

    Ok(())
}
