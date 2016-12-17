mod viewer;

use errors::Result;
use nitro::Bmd;

pub fn main(bmd: &Bmd) -> Result<()> {
    let model = &bmd.mdl.models[0];
    viewer::viewer(model, &bmd.tex)?;
    Ok(())
}
