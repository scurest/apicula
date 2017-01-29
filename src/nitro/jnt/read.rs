use errors::Result;
use nitro::info_block;
use nitro::jnt::Animation;
use nitro::jnt::Jnt;
use nitro::jnt::Object;
use nitro::jnt::Rotation;
use nitro::jnt::Scaling;
use nitro::jnt::ScalingData;
use nitro::jnt::Timing;
use nitro::jnt::Translation;
use nitro::jnt::TranslationData;
use nitro::name::Name;
use util::bits::BitField;
use util::cur::Cur;

pub fn read_jnt(cur: Cur) -> Result<Jnt> {
    fields!(cur, JNT0 {
        stamp: [u8; 4],
        section_size: u32,
        end: Cur,
    });
    check!(stamp == b"JNT0")?;

    let animations = info_block::read::<u32>(end)?
        .map(|(off, name)| read_animation((cur + off as usize)?, name))
        .collect::<Result<_>>()?;

    Ok(Jnt {
        animations: animations,
    })
}

fn read_animation(cur: Cur, name: Name) -> Result<Animation> {
    fields!(cur, animation {
        stamp: [u8; 4],
        num_frames: u16,
        num_objects: u16,
        unknown: u32,
        pivot_data_off: u32,
        basis_data_off: u32,
        object_offs: [u16; num_objects],
    });
    check!(stamp == b"J\0AC")?; // I don't think the DS actually reads that bizarre NUL byte

    let pivot_data = (cur + pivot_data_off as usize)?;
    let basis_data = (cur + basis_data_off as usize)?;

    let objects = object_offs
        .map(|off| read_object((cur + off as usize)?, cur))
        .collect::<Result<_>>()?;

    Ok(Animation {
        name: name,
        num_frames: num_frames,
        pivot_data: pivot_data,
        basis_data: basis_data,
        objects: objects,
    })
}

fn read_object<'a>(mut cur: Cur<'a>, anim_cur: Cur<'a>) -> Result<Object<'a>> {
    fields!(cur, anim_object {
        flags: u16,
        dummy: u8,
        id: u8,
        end: Cur,
    });
    cur = end;

    let mut trans_x = None;
    let mut trans_y = None;
    let mut trans_z = None;
    let mut rotation = None;
    let mut scale_x = None;
    let mut scale_y = None;
    let mut scale_z = None;

    let has_any_transform = flags.bits(0,1);
    let has_translation = flags.bits(1,2);
    let translation_fixed_or_varying = flags.bits(3,6);
    let has_rotation = flags.bits(6,7);
    let rotation_fixed_or_varying = flags.bits(8,9);
    let has_scale = flags.bits(9,10);
    let scale_fixed_or_varying = flags.bits(11,14);
    // TODO: what do bits 2, 7, and 10 mean?

    if has_any_transform == 0 {
        if has_translation == 0 {
            let is_fixed = translation_fixed_or_varying.bits(0,1) == 1;
            trans_x = Some(read_trans(&mut cur, anim_cur, is_fixed)?);
            let is_fixed = translation_fixed_or_varying.bits(1,2) == 1;
            trans_y = Some(read_trans(&mut cur, anim_cur, is_fixed)?);
            let is_fixed = translation_fixed_or_varying.bits(2,3) == 1;
            trans_z = Some(read_trans(&mut cur, anim_cur, is_fixed)?);
        }
        if has_rotation == 0 {
            let is_fixed = rotation_fixed_or_varying.bits(0,1) == 1;
            rotation = Some(read_rotation(&mut cur, anim_cur, is_fixed)?);
        }
        if has_scale == 0 {
            let is_fixed = scale_fixed_or_varying.bits(0,1) == 1;
            scale_x = Some(read_scaling(&mut cur, anim_cur, is_fixed)?);
            let is_fixed = scale_fixed_or_varying.bits(1,2) == 1;
            scale_y = Some(read_scaling(&mut cur, anim_cur, is_fixed)?);
            let is_fixed = scale_fixed_or_varying.bits(2,3) == 1;
            scale_z = Some(read_scaling(&mut cur, anim_cur, is_fixed)?);
        }
    }

    Ok(Object {
        trans_x: trans_x,
        trans_y: trans_y,
        trans_z: trans_z,
        rotation: rotation,
        scale_x: scale_x,
        scale_y: scale_y,
        scale_z: scale_z,
    })
}

fn read_trans<'a>(cur: &mut Cur<'a>, anim_cur: Cur<'a>, is_fixed: bool) -> Result<Translation<'a>> {
    let res = if is_fixed {
        Translation::Fixed(cur.next::<u32>()?)
    } else {
        let params = cur.next::<u32>()?;
        let off = cur.next::<u32>()?;

        let timing = timing_from_params(params);

        let data_width = params.bits(28,30);
        let data_len = data_len(&timing);
        let data = match data_width {
            0 => TranslationData::Full((anim_cur + off as usize)?.next_n::<u32>(data_len)?),
            _ => TranslationData::Half((anim_cur + off as usize)?.next_n::<u16>(data_len)?),
        };

        Translation::Varying {
            timing: timing,
            data: data,
        }
    };
    trace!("translation: {:?}", res);
    Ok(res)
}

fn read_rotation<'a>(cur: &mut Cur<'a>, anim_cur: Cur<'a>, is_fixed: bool) -> Result<Rotation<'a>> {
    let res = if is_fixed {
        let x = cur.next::<u16>()?;
        let _ = cur.next::<u16>()?; // padding for alignment, surely
        Rotation::Fixed(x)
    } else {
        let params = cur.next::<u32>()?;
        let off = cur.next::<u32>()?;

        let timing = timing_from_params(params);

        let data_len = data_len(&timing);
        let data = (anim_cur + off as usize)?.next_n::<u16>(data_len)?;

        Rotation::Varying {
            timing: timing,
            data: data,
        }
    };
    trace!("rotation: {:?}", res);
    Ok(res)
}

fn read_scaling<'a>(cur: &mut Cur<'a>, anim_cur: Cur<'a>, is_fixed: bool) -> Result<Scaling<'a>> {
    let res = if is_fixed {
        Scaling::Fixed(cur.next::<(u32, u32)>()?)
    } else {
        let params = cur.next::<u32>()?;
        let off = cur.next::<u32>()?;

        let timing = timing_from_params(params);

        let data_width = params.bits(28,30);
        let data_len = data_len(&timing);
        let data = match data_width {
            0 => ScalingData::Full((anim_cur + off as usize)?.next_n::<(u32,u32)>(data_len)?),
            _ => ScalingData::Half((anim_cur + off as usize)?.next_n::<(u16,u16)>(data_len)?),
        };

        Scaling::Varying {
            timing: timing,
            data: data,
        }
    };
    trace!("scale: {:?}", res);
    Ok(res)
}

fn timing_from_params(params: u32) -> Timing {
    let start_frame = params.bits(0,16) as u16;
    let end_frame = params.bits(16,28) as u16;
    let speed = params.bits(30,32) as u8;
    Timing {
        start_frame: start_frame,
        end_frame: end_frame,
        speed: speed,
    }
}

/// Number of data for a varying component.
fn data_len(timing: &Timing) -> usize {
    // FIXME: check how this works
    ((timing.end_frame - timing.start_frame) >> timing.speed) as usize
}
