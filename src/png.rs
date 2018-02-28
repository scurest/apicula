use png_crate::HasParameters;
use std::ffi::OsStr;
use errors::Result;

pub fn write_rgba<S: AsRef<OsStr>>(s: &S, rgba: &[u8], width: u32, height: u32)
-> Result<()>
{
    use std::fs::File;
    use std::path::Path;
    use png_crate::{ColorType, BitDepth, Encoder};

    let f = File::create(&Path::new(s))?;
    let mut encoder = Encoder::new(f, width, height);
    encoder
        .set(ColorType::RGBA)
        .set(BitDepth::Eight);

    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;

    Ok(())
}
