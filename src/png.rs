use errors::Result;
use pnglib;
use pnglib::HasParameters;
use std::ffi::OsStr;
use std::fs::File;
use std::path::Path;

pub fn write<S: AsRef<OsStr>>(s: &S, rgba: &[u8], width: u32, height: u32) -> Result<()> {
    let fout = File::create(&Path::new(s))?;
    let mut enc = pnglib::Encoder::new(fout, width, height);
    enc.set(pnglib::ColorType::RGBA).set(pnglib::BitDepth::Eight);
    let mut writer = enc.write_header()?;
    writer.write_image_data(rgba)?;
    Ok(())
}
