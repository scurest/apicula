use cgmath::{Matrix3, Matrix4};
use crate::util::bits::BitField;
use crate::util::cur::Cur;
use crate::util::fixed::{fix16, fix32};
use crate::nitro::Name;
use crate::nitro::rotation::{pivot_mat, basis_mat};
use std::ops::{Mul, Add};
use crate::errors::Result;

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

    // A curve defined by sampling at a fixed rate on the interval [start_frame,
    // end_frame].
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

    if num_frames == 0 {
        bail!("ignoring animation with 0 frames");
    }

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

        // The flags tells us if anything is anything, which of the TRS
        // properties are animated, and for each animated property, whether its
        // curve is constant or not.

        #[derive(Debug)]
        struct AnimationFlags {
            animated: bool,
            trans_animated: bool,
            trans_xyz_const: [bool; 3],
            rot_animated: bool,
            rot_const: bool,
            scale_animated: bool,
            scale_xyz_const: [bool; 3],
        }

        let flags = AnimationFlags {
            animated: flags.bits(0,1) == 0,
            trans_animated: flags.bits(1,3) == 0,
            trans_xyz_const: [
                flags.bits(3,4) != 0,
                flags.bits(4,5) != 0,
                flags.bits(5,6) != 0,
            ],
            rot_animated: flags.bits(6,8) == 0,
            rot_const: flags.bits(8,9) != 0,
            scale_animated: flags.bits(9,11) == 0,
            scale_xyz_const: [
                flags.bits(11,12) != 0,
                flags.bits(12,13) != 0,
                flags.bits(13,14) != 0,
            ]
        };

        trace!("flags: {:?}", flags);

        let mut trs_curves = TRSCurves {
            trans: [Curve::None, Curve::None, Curve::None],
            rotation: Curve::None,
            scale: [Curve::None, Curve::None, Curve::None],
        };

        if !flags.animated {
            return Ok(trs_curves);
        }

        ////////////////
        // Translation
        ////////////////

        if flags.trans_animated {
            for i in 0..3 {
                let is_const = flags.trans_xyz_const[i];
                if is_const {
                    let v = fix32(cur.next::<u32>()?, 1, 19, 12);
                    trs_curves.trans[i as usize] = Curve::Constant(v);
                } else {
                    let info = CurveInfo::from_u32(cur.next::<u32>()?)?;
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
        // (see above, these were stored in the parent J0AC) where the values
        // are located. This lambda is used to get the actual values.
        let fetch_matrix = |x: u16| -> Result<Matrix3<f64>> {
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

        if flags.rot_animated {
            if flags.rot_const {
                let v = cur.next::<u16>()?;
                let _ = cur.next::<u16>()?; // Skipped? For alignment?
                trs_curves.rotation = Curve::Constant(fetch_matrix(v)?);
            } else {
                let info = CurveInfo::from_u32(cur.next::<u32>()?)?;
                let off = cur.next::<u32>()?;

                let start_frame = info.start_frame;
                let end_frame = info.end_frame;
                let values = {
                    // Do this with an explicit with_capacity + push loop
                    // because collecting an iterator into a Result doesn't
                    // reserve the capacity in advance.
                    // See rust-lang/rust/#48994.
                    let num_samples = info.num_samples();
                    let mut samples: Vec<Matrix3<f64>> =
                        Vec::with_capacity(num_samples);
                    for v in (base_cur + off).next_n::<u16>(num_samples)? {
                        samples.push(fetch_matrix(v)?);
                    }
                    samples
                };

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

        if flags.scale_animated {
            for i in 0..3 {
                let is_const = flags.scale_xyz_const[i];
                if is_const {
                    let v = fix32(cur.next::<(u32, u32)>()?.0, 1, 19, 12);
                    trs_curves.scale[i as usize] = Curve::Constant(v);
                } else {
                    let info = CurveInfo::from_u32(cur.next::<u32>()?)?;
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
    fn from_u32(x: u32) -> Result<CurveInfo> {
        let start_frame = x.bits(0, 16) as u16;
        let end_frame = x.bits(16, 28) as u16;
        let rate = x.bits(30, 32) as u8;
        let data_width = x.bits(28, 30) as u8;

        check!(start_frame < end_frame)?;

        Ok(CurveInfo { start_frame, end_frame, rate, data_width })
    }

    fn num_samples(&self) -> usize {
        // XXX check this (I literally just made it up)
        ((self.end_frame - self.start_frame) >> self.rate) as usize
    }
}


impl<T> Curve<T>
where T: Copy + Mul<f64, Output=T> + Add<T, Output=T> {
    pub fn sample_at(&self, default: T, frame: u16) -> T {
        match *self {
            Curve::None => default,
            Curve::Constant(v) => { v },
            Curve::Samples { start_frame, end_frame, ref values } => {
                if values.is_empty() { return default; }

                // XXX what's the behavior outside the defined bounds?
                // We're currently using "hold value".
                // TODO Worry about end_frame == 0?
                if frame <= start_frame { return values[0]; }
                if frame >= end_frame - 1 { return values[values.len() - 1]; }

                // Linearly interpolate between the two nearest values
                // XXX I made this up too :b
                let lam = (frame - start_frame) as f64 / (end_frame - 1 - start_frame) as f64;
                let idx = lam * (values.len() - 1) as f64;
                let lo_idx = idx.floor();
                let hi_idx = idx.ceil();
                let gamma = idx - lo_idx;
                values[lo_idx as usize] * (1.0 - gamma) + values[hi_idx as usize] * gamma
            }
        }
    }
}


impl TRSCurves {
    pub fn sample_at(&self, frame: u16) -> Matrix4<f64> {
        use cgmath::{One, vec3};

        let tx = self.trans[0].sample_at(0.0, frame);
        let ty = self.trans[1].sample_at(0.0, frame);
        let tz = self.trans[2].sample_at(0.0, frame);
        let rot = self.rotation.sample_at(Matrix3::one(), frame);
        let sx = self.scale[0].sample_at(1.0, frame);
        let sy = self.scale[1].sample_at(1.0, frame);
        let sz = self.scale[2].sample_at(1.0, frame);

        let translation = Matrix4::from_translation(vec3(tx, ty, tz));
        let rotation = Matrix4::from(rot);
        let scale = Matrix4::from_nonuniform_scale(sx, sy, sz);

        translation * rotation * scale
    }
}
