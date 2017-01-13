use cgmath::Matrix3;
use cgmath::Matrix4;
use cgmath::One;
use cgmath::vec3;
use errors::Result;
use nitro::jnt::Animation;
use nitro::jnt::Object;
use nitro::jnt::Rotation;
use nitro::jnt::Scaling;
use nitro::jnt::ScalingData;
use nitro::jnt::Timing;
use nitro::jnt::Translation;
use nitro::jnt::TranslationData;
use nitro::mdl::pivot_mat;
use std::cmp;
use util::bits::BitField;
use util::fixed::fix16;
use util::fixed::fix32;

fn frame_to_index(timing: &Timing, frame: u16) -> Option<usize> {
    if timing.end_frame == timing.start_frame {
        return None;
    }

    let frame = cmp::max(cmp::min(frame, timing.end_frame - 1), timing.start_frame);
    Some(((frame - timing.start_frame) >> timing.speed) as usize)
}

fn get_pivot(anim: &Animation, idx: u16) -> Result<Matrix4<f64>> {
    let (selneg, a, b) = anim.pivot_data.nth::<(u16, u16, u16)>(idx as usize)?;
    let sel = selneg.bits(0,4);
    let neg = selneg.bits(4,8);
    let a = fix16(a, 1, 3, 12);
    let b = fix16(b, 1, 3, 12);
    pivot_mat(sel, neg, a, b)
}

fn get_basis(anim: &Animation, idx: u16) -> Result<Matrix4<f64>> {
    let d = anim.basis_data.nth::<(u16, u16, u16, u16, u16)>(idx as usize)?;

    let a1 = fix16(d.0.bits(3,16), 1, 0, 12);
    let a2 = fix16(d.1.bits(3,16), 1, 0, 12);
    let a3 = fix16(d.2.bits(3,16), 1, 0, 12);
    let a = vec3(a1, a2, a3);

    let b1 = fix16(d.3.bits(3,16), 1, 0, 12);
    let b2 = fix16(d.4.bits(3,16), 1, 0, 12);
    let b3_val =
        (d.4.bits(0,3) << 12) |
        (d.0.bits(0,3) << 9) |
        (d.1.bits(0,3) << 6) |
        (d.2.bits(0,3) << 3) |
        (d.3.bits(0,3) << 0);
    let b3 = fix16(b3_val, 1, 0, 12);
    let b = vec3(b1, b2, b3);

    let c = a.cross(b);

    Ok(Matrix3::from_cols(a, b, c).into())
}

pub fn to_matrix<'a>(object: &Object<'a>, anim: &Animation<'a>, frame: u16) -> Result<Matrix4<f64>> {
    let decode_trans = |trans: &Translation| {
        Some(match *trans {
            Translation::Fixed(x) => fix32(x, 1, 19, 12),
            Translation::Varying { ref timing, ref data } => {
                let idx = match frame_to_index(timing, frame) {
                    Some(idx) => idx,
                    None => return None,
                };
                match *data {
                    TranslationData::Half(v) => fix16(v.get(idx), 1, 3, 12),
                    TranslationData::Full(v) => fix32(v.get(idx), 1, 19, 12),
                }
            }
        })
    };

    let decode_rot = |rot: &Rotation| {
        let select = match *rot {
            Rotation::Fixed(x) => x,
            Rotation::Varying { ref timing, ref data } => {
                let idx = match frame_to_index(timing, frame) {
                    Some(idx) => idx,
                    None => return None,
                };
                data.get(idx)
            }
        };

        let mode = select.bits(15,16);
        let idx = select.bits(0,15);
        Some(match mode {
            1 => get_pivot(anim, idx),
            _ => get_basis(anim, idx),
        })
    };

    let decode_scale = |scale: &Scaling| {
        Some(match *scale {
            Scaling::Fixed((x,_)) => fix32(x, 1, 19, 12),
            Scaling::Varying { ref timing, ref data } => {
                let idx = match frame_to_index(timing, frame) {
                    Some(idx) => idx,
                    None => return None,
                };
                match *data {
                    ScalingData::Half(v) => fix16(v.get(idx).0, 1, 3, 12),
                    ScalingData::Full(v) => fix32(v.get(idx).0, 1, 19, 12),
                }
            }
        })
    };

    let tx = object.trans_x.as_ref().and_then(&decode_trans).unwrap_or(0.0);
    let ty = object.trans_y.as_ref().and_then(&decode_trans).unwrap_or(0.0);
    let tz = object.trans_z.as_ref().and_then(&decode_trans).unwrap_or(0.0);
    let trans = Matrix4::from_translation(vec3(tx, ty, tz));

    let rot = object.rotation.as_ref().and_then(&decode_rot).unwrap_or(Ok(Matrix4::one()))?;

    let sx = object.scale_x.as_ref().and_then(&decode_scale).unwrap_or(1.0);
    let sy = object.scale_y.as_ref().and_then(&decode_scale).unwrap_or(1.0);
    let sz = object.scale_z.as_ref().and_then(&decode_scale).unwrap_or(1.0);
    let scale = Matrix4::from_nonuniform_scale(sx, sy, sz);

    Ok(trans * rot * scale)
}
