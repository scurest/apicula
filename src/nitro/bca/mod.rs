mod anim;
pub mod object;

pub use self::anim::read_bca;
pub use self::anim::read_jnt;

use util::cur::Cur;
use util::name::Name;
use util::view::View;

#[derive(Debug, Clone)]
pub struct Bca<'a> {
    pub jnts: Vec<Jnt<'a>>,
}

#[derive(Debug, Clone)]
pub struct Jnt<'a> {
    pub animations: Vec<Animation<'a>>,
}

#[derive(Debug, Clone)]
pub struct Animation<'a> {
    pub name: Name,
    pub num_frames: u16,
    pub pivot_data: Cur<'a>,
    pub basis_data: Cur<'a>,
    pub objects: Vec<Object<'a>>,
}

#[derive(Debug, Clone)]
pub struct Object<'a> {
    pub trans_x: Option<Translation<'a>>,
    pub trans_y: Option<Translation<'a>>,
    pub trans_z: Option<Translation<'a>>,
    pub rotation: Option<Rotation<'a>>,
    pub scale_x: Option<Scaling<'a>>,
    pub scale_y: Option<Scaling<'a>>,
    pub scale_z: Option<Scaling<'a>>,
}

#[derive(Debug, Clone)]
pub struct Timing {
    start_frame: u16,
    end_frame: u16,
    speed: u8,
}

#[derive(Debug, Clone)]
pub enum Translation<'a> {
    Fixed(u32),
    Varying {
        timing: Timing,
        data: TranslationData<'a>,
    },
}

#[derive(Debug, Clone)]
pub enum TranslationData<'a> {
    Half(View<'a, u16>),
    Full(View<'a, u32>),
}

#[derive(Debug, Clone)]
pub enum Rotation<'a> {
    Fixed(u16),
    Varying {
        timing: Timing,
        data: View<'a, u16>,
    },
}

#[derive(Debug, Clone)]
pub enum Scaling<'a> {
    Fixed((u32, u32)),
    Varying {
        timing: Timing,
        data: ScalingData<'a>,
    },
}

#[derive(Debug, Clone)]
pub enum ScalingData<'a> {
    Half(View<'a, (u16, u16)>),
    Full(View<'a, (u32, u32)>),
}
