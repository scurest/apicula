use cgmath::{Matrix3, Matrix4};
use util::bits::BitField;
use util::{cur::Cur, fixed::{fix16, fix32}};
use nitro::{Name, rotation::{pivot_mat, basis_mat}};
use errors::Result;

pub struct Animation {
    pub name: Name,
    pub num_frames: u16,
    pub objects_curves: Vec<TRSCurves>,
}

pub struct TRSCurves {
    pub trans: [Curve<f64>; 3],
    pub rotation: Curve<Matrix3<f64>>,
    pub scale: [Curve<f64>; 3],
}

pub enum Curve<T> {
    // A curve which is everywhere undefined. (Sampling at an undefined point
    // should produce an appropriate default value, like 0.0 for a translation).
    None,

    // A curve which has a constant value for all time.
    //
    //      |
    //      |-------------
    //      |
    //      |_____________
    //
    Constant(T),

    // A curve defined by sampling at a fixed rate. Its domain is the interval
    // [start_frame, end_frame].
    //
    //      |    ,-,     _
    //      |  ,'| |\  ,'|`,
    //      |  | | | `-| | |
    //      |__|_|_|_|_|_|_|_
    //         |           |
    //     start_frame   end_frame
    //
    Samples {
        start_frame: u16,
        end_frame: u16,
        values: Vec<T>,
    }
}


pub fn read_animation(base_cur: Cur, name: Name) -> Result<Animation> {
    fields!(base_cur, animation {
        stamp: [u8; 4],
        num_frames: u16,
        num_objects: u16,
        unknown: u32,
        pivot_data_off: u32,
        basis_data_off: u32,
        object_offs: [u16; num_objects],
    });

    check!(stamp == b"J\0AC")?; // wtf NUL

    let pivot_data = base_cur + pivot_data_off;
    let basis_data = base_cur + basis_data_off;

    let objects_curves = object_offs.map(|curves_off| {
        let mut cur = base_cur + curves_off;
        fields!(cur, object_curves {
            flags: u16,
            dummy: u8,
            index: u8,
            end: Cur,
        });
        cur = end;

        let mut trs_curves = TRSCurves {
            trans: [Curve::None, Curve::None, Curve::None],
            rotation: Curve::None,
            scale: [Curve::None, Curve::None, Curve::None],
        };

        let no_curves = flags.bits(0,1) != 0;
        if no_curves {
            return Ok(trs_curves);
        }


        ////////////////
        // Translation
        ////////////////

        let no_trans = flags.bits(1,2) != 0;
        if !no_trans {
            for i in 0..3 {
                let is_const = flags.bits(3+i, 4+i) != 0;
                if is_const {
                    let v = fix32(cur.next::<u32>()?, 1, 19, 12);
                    trs_curves.trans[i as usize] = Curve::Constant(v);
                } else {
                    let info = CurveInfo::from_u32(cur.next::<u32>()?);
                    let off = cur.next::<u32>()?;

                    let start_frame = info.start_frame;
                    let end_frame = info.end_frame;
                    let values = match info.data_width {
                        0 => (base_cur + off)
                            .next_n::<u32>(info.num_samples())?
                            .map(|x| fix32(x, 1, 19, 12))
                            .collect::<Vec<f64>>(),

                        _ => (base_cur + off)
                            .next_n::<u16>(info.num_samples())?
                            .map(|x| fix16(x, 1, 3, 12))
                            .collect::<Vec<f64>>(),
                    };

                    trs_curves.trans[i as usize] = Curve::Samples {
                        start_frame, end_frame, values,
                    };
                }
            }
        }


        /////////////
        // Rotation
        /////////////

        // In this case, the data at base_cur + off doesn't store the actual
        // curve values, it stores references into pivot_data and basis_data
        // (see above, there were stored in the parent J0AC)  where the values
        // are located. This is used to get the actual values.
        let decode = |x: u16| -> Result<Matrix3<f64>> {
            let mode = x.bits(15, 16);
            let idx = x.bits(0, 15) as usize;
            Ok(match mode {
                1 => {
                    // Pivot data, just like in the MDL model files.
                    let (selneg, a, b) = pivot_data.nth::<(u16, u16, u16)>(idx)?;
                    let sel = selneg.bits(0, 4);
                    let neg = selneg.bits(4, 8);
                    let a = fix16(a, 1, 3, 12);
                    let b = fix16(b, 1, 3, 12);
                    pivot_mat(sel, neg, a, b)
                }
                _ => {
                    let d = basis_data.nth::<(u16, u16, u16, u16, u16)>(idx as usize)?;
                    basis_mat(d)
                }
            })

        };

        let no_rotation = flags.bits(6, 7) != 0;
        if !no_rotation {
            let is_const = flags.bits(8, 9) != 0;
            if is_const {
                let v = cur.next::<u16>()?;
                let _ = cur.next::<u16>()?; // Skipped? For alignment?
                trs_curves.rotation = Curve::Constant(decode(v)?);
            } else {
                let info = CurveInfo::from_u32(cur.next::<u32>()?);
                let off = cur.next::<u32>()?;

                let start_frame = info.start_frame;
                let end_frame = info.end_frame;
                let values =
                    (base_cur + off)
                    .next_n::<u16>(info.num_samples())?
                    .map(decode)
                    .collect::<Result<Vec<Matrix3<f64>>>>()?;

                trs_curves.rotation = Curve::Samples {
                    start_frame, end_frame, values,
                };
            }
        }


        //////////
        // Scale
        //////////

        // These are just like translations except there are two values per
        // curve instead of one. I ignore the second one because I don't know
        // what it's for.

        let no_scale = flags.bits(9, 10) != 0;
        if !no_scale {
            for i in 0..3 {
                let is_const = flags.bits(11+i, 12+i) != 0;
                if is_const {
                    let v = fix32(cur.next::<(u32, u32)>()?.0, 1, 19, 12);
                    trs_curves.scale[i as usize] = Curve::Constant(v);
                } else {
                    let info = CurveInfo::from_u32(cur.next::<u32>()?);
                    let off = cur.next::<u32>()?;

                    let start_frame = info.start_frame;
                    let end_frame = info.end_frame;
                    let values = match info.data_width {
                        0 => (base_cur + off)
                            .next_n::<(u32, u32)>(info.num_samples())?
                            .map(|(x, _)| fix32(x, 1, 19, 12))
                            .collect::<Vec<f64>>(),

                        _ => (base_cur + off)
                            .next_n::<(u16, u16)>(info.num_samples())?
                            .map(|(x, _)| fix16(x, 1, 3, 12))
                            .collect::<Vec<f64>>(),
                    };

                    trs_curves.scale[i as usize] = Curve::Samples {
                        start_frame, end_frame, values,
                    };
                }
            }
        }

        // Finally finished! TT_TT

        Ok(trs_curves)

    }).collect::<Result<Vec<TRSCurves>>>()?;

    Ok(Animation { name, num_frames, objects_curves })
}


struct CurveInfo {
    start_frame: u16,
    end_frame: u16,
    rate: u8,
    data_width: u8,
}

impl CurveInfo {
    fn from_u32(x: u32) -> CurveInfo {
        let start_frame = x.bits(0, 16) as u16;
        let end_frame = x.bits(16, 28) as u16;
        let rate = x.bits(30, 32) as u8;
        let data_width = x.bits(28, 30) as u8;
        CurveInfo { start_frame, end_frame, rate, data_width }
    }

    fn num_samples(&self) -> usize {
        // FIXME: check this (I literally just made it up)
        ((self.end_frame - self.start_frame) >> self.rate) as usize
    }
}


impl TRSCurves {
    pub fn sample_at(&self, frame: u16) -> Matrix4<f64> {
        use cgmath::{One, vec3};

        fn sample_curve<T: Copy>(curve: &Curve<T>, default: T, frame: u16) -> T {
            match *curve {
                Curve::None => default,
                Curve::Constant(v) => { v },
                Curve::Samples { start_frame, end_frame, ref values } => {
                    // TODO what's the behavior outside the defined bounds?
                    if frame <= start_frame { return default; }
                    if frame >= end_frame { return default; }
                    let lam = (frame - start_frame) as f32 / (end_frame - start_frame) as f32;
                    let idx = (lam * (values.len() - 1) as f32).round() as usize;
                    values[idx]
                }
            }
        }

        let tx = sample_curve(&self.trans[0], 0.0, frame);
        let ty = sample_curve(&self.trans[1], 0.0, frame);
        let tz = sample_curve(&self.trans[2], 0.0, frame);
        let rot = sample_curve(&self.rotation, Matrix3::one(), frame);
        let sx = sample_curve(&self.scale[0], 1.0, frame);
        let sy = sample_curve(&self.scale[1], 1.0, frame);
        let sz = sample_curve(&self.scale[2], 1.0, frame);

        let translation = Matrix4::from_translation(vec3(tx, ty, tz));
        let rotation = Matrix4::from(rot);
        let scale = Matrix4::from_nonuniform_scale(sx, sy, sz);

        translation * rotation * scale
    }
}
