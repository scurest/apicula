// HIGHLY INCOMPLETE!!!

use super::animation::Curve;
use crate::util::cur::Cur;
use crate::util::view::Viewable;
use crate::util::fixed::fix16;
use cgmath::{Matrix4, vec3};
use crate::nitro::Name;
use crate::errors::Result;
use super::info_block;

/// Material animation. Does things like UV matrix animation.
pub struct MaterialAnimation {
    pub name: Name,
    pub num_frames: u16,
    pub tracks: Vec<MaterialTrack>,
}

/// Targets one material in a model and animates it.
pub struct MaterialTrack {
    pub name: Name,
    pub channels: [MaterialChannel; 5],
}

/// Targets one material property for animation????
pub struct MaterialChannel {
    pub num_frames: u16,
    pub target: MatChannelTarget,
    pub curve: Curve<f64>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum MatChannelTarget {
    TranslationU,
    TranslationV,
    Unknown,
}

pub fn read_mat_anim(cur: Cur, name: Name) -> Result<MaterialAnimation> {
    debug!("material animation: {:?}", name);

    fields!(cur, mat_anim {
        _unknown: [u8; 4],  // b'M\0AT' ??
        num_frames: u16,
        unknown: u16,
        end: Cur,
    });

    let tracks = info_block::read::<[ChannelData; 5]>(end)?
        .map(|(chans, name)| {
            let c0 = read_channel(cur, chans[0], MatChannelTarget::Unknown)?;
            let c1 = read_channel(cur, chans[1], MatChannelTarget::Unknown)?;
            let c2 = read_channel(cur, chans[2], MatChannelTarget::Unknown)?;
            let c3 = read_channel(cur, chans[3], MatChannelTarget::TranslationU)?;
            let c4 = read_channel(cur, chans[4], MatChannelTarget::TranslationV)?;
            let channels = [c0, c1, c2, c3, c4];
            Ok(MaterialTrack { name, channels })
        })
        .collect::<Result<Vec<MaterialTrack>>>()?;

    Ok(MaterialAnimation {
        name,
        num_frames,
        tracks,
    })
}

#[derive(Debug, Copy, Clone)]
struct ChannelData {
    num_frames: u16,
    flags: u8, // some sort of flags
    offset: u32, // meaning appears to depend on flags
}
impl Viewable for ChannelData {
    fn size() -> usize { 8 }
    fn view(buf: &[u8]) -> ChannelData {
        let mut cur = Cur::new(buf);
        let num_frames = cur.next::<u16>().unwrap();
        let _dummy = cur.next::<u8>().unwrap(); // always 0?
        let flags = cur.next::<u8>().unwrap();
        let offset = cur.next::<u32>().unwrap();
        ChannelData { num_frames, flags, offset }
    }
}

fn read_channel(base_cur: Cur, data: ChannelData, target: MatChannelTarget) -> Result<MaterialChannel> {
    if !((data.flags == 16) && target != MatChannelTarget::Unknown) {
        // That's the only case I understand; otherwise bail.
        return Ok(MaterialChannel {
            num_frames: 0,
            target,
            curve: Curve::None,
        });
    }

    let values = (base_cur + data.offset)
        .next_n::<u16>(data.num_frames as usize)?
        .map(|n| fix16(n, 1, 10, 5))
        .collect::<Vec<f64>>();
    let curve = Curve::Samples {
        start_frame: 0,
        end_frame: data.num_frames,
        values,
    };

    Ok(MaterialChannel {
        num_frames: data.num_frames,
        target,
        curve,
    })
}

impl MaterialTrack {
    pub fn eval_uv_mat(&self, frame: u16) -> Matrix4<f64> {
        let (mut u, mut v) = (0.0, 0.0);
        for chan in &self.channels {
            if chan.target == MatChannelTarget::TranslationU {
                u = chan.curve.sample_at(u, frame);
            }
            if chan.target == MatChannelTarget::TranslationV {
                v = chan.curve.sample_at(v, frame);
            }
        }
        Matrix4::from_translation(vec3(u, v, 0.0))
    }
}
