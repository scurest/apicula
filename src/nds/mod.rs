//! NDS hardware functions.

pub mod gpu_cmds;
pub mod texture_formats;
pub mod texture_params;

pub use self::texture_formats::{TextureFormat, FormatDesc, Alpha};
pub use self::texture_params::TextureParams;
