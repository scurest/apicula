/// Converts a set of Nitro TRS curves into a form suitable for glTF.
///
/// Nitro files contain one curve for each translation/scale component, while
/// glTF contains one curve for the whole 3-vector, so we need to resample the
/// three component curve onto their common domain and join them together. And
/// we need to convert the curve of rotation matrices to a curve of quaternions.

use nitro::animation::{TRSCurves, Curve};
use cgmath::{Vector3, Matrix3, Quaternion, InnerSpace, vec3};

/// Represents the domain of a Curve.
///
/// NOTE: we interpret the domain of a constant curve as being a curve with a
/// single sample at 0. IOW we basically forget about constant curves in this
/// module.
#[derive(Copy, Clone)]
pub enum CurveDomain {
    None,
    Sampled {
        start_frame: u16,
        end_frame: u16,
        sampling_rate: u16,
    }
}

impl<T> Curve<T> {
    pub fn domain(&self) -> CurveDomain {
        match *self {
            Curve::None => CurveDomain::None,
            Curve::Constant(_) => CurveDomain::Sampled {
                start_frame: 0,
                end_frame: 1,
                sampling_rate: 1,
            },
            Curve::Samples {
                start_frame,
                end_frame,
                ref values
            } => CurveDomain::Sampled {
                start_frame,
                end_frame,
                sampling_rate: (end_frame - start_frame) / values.len() as u16,
            }
        }
    }
}

impl CurveDomain {
    pub fn union(self, other: CurveDomain) -> CurveDomain {
        match (self, other) {
            (CurveDomain::None, d) => d,
            (d, CurveDomain::None) => d,
            (
                CurveDomain::Sampled { start_frame: s1, end_frame: e1, sampling_rate: r1 },
                CurveDomain::Sampled { start_frame: s2, end_frame: e2, sampling_rate: r2 },
            ) => CurveDomain::Sampled {
                start_frame: s1.min(s2),
                end_frame: e1.max(e2),
                sampling_rate: r1.min(r2),
            }
        }
    }
}

pub struct GlTFObjectCurves {
    pub translation: Curve<Vector3<f64>>,
    pub rotation: Curve<Quaternion<f64>>,
    pub scale: Curve<Vector3<f64>>,
}

impl GlTFObjectCurves {
    pub fn for_trs_curves(trs_curves: &TRSCurves) -> GlTFObjectCurves {
        let translation = resample_vec3(&trs_curves.trans, 0.0);
        let rotation = rotation_curve(&trs_curves.rotation);
        let scale = resample_vec3(&trs_curves.scale, 1.0);

        GlTFObjectCurves { translation, rotation, scale }
    }
}

/// Turns an array of three curves of reals into one curve of 3-vectors by
/// re-sampling the curves onto their common domain.
fn resample_vec3(curves: &[Curve<f64>; 3], default_value: f64) -> Curve<Vector3<f64>> {
    let resampled_domain =
        curves[0].domain()
        .union(curves[1].domain())
        .union(curves[2].domain());

    match resampled_domain {
        CurveDomain::None => Curve::None,
        CurveDomain::Sampled { start_frame, end_frame, sampling_rate } => {
            let num_samples = (end_frame - start_frame) / sampling_rate;
            let mut values = Vec::with_capacity(num_samples as usize);

            let mut frame = start_frame;
            while frame < end_frame {
                let x = curves[0].sample_at(default_value, frame);
                let y = curves[1].sample_at(default_value, frame);
                let z = curves[2].sample_at(default_value, frame);
                values.push(vec3(x, y, z));

                frame += sampling_rate;
            }

            Curve::Samples { start_frame, end_frame, values }
        }
    }
}

/// Turns a curve of rotation matrices into a curve of quaternions.
fn rotation_curve(matrix_curve: &Curve<Matrix3<f64>>) -> Curve<Quaternion<f64>> {
    fn to_quat(m: Matrix3<f64>) -> Quaternion<f64> {
        Quaternion::from(m).normalize()
    }

    match *matrix_curve {
        Curve::None => Curve::None,
        Curve::Constant(m) => Curve::Samples {
            start_frame: 0,
            end_frame: 1,
            values: vec![to_quat(m)],
        },
        Curve::Samples { start_frame, end_frame, ref values } => {
            let mut quats = values.iter()
                .map(|&m| to_quat(m))
                .collect::<Vec<Quaternion<f64>>>();

            // Ensure the quaternions go on the shortest path through the
            // hypersphere
            for i in 1..quats.len() {
                if quats[i].dot(quats[i-1]) < 0.0 {
                    quats[i] = -quats[i];
                }
            }

            Curve::Samples { start_frame, end_frame, values: quats }
        }
    }
}
