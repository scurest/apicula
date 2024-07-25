use crate::nitro::Model;
use cgmath::{Vector3, Quaternion, InnerSpace, One, vec3, Matrix4};

pub struct TRS {
    pub translation: Option<Vector3<f64>>,
    pub rotation_quaternion: Option<Quaternion<f64>>,
    pub scale: Option<Vector3<f64>>,
}

pub struct ObjectTRSes {
    pub objects: Vec<TRS>,
}

impl ObjectTRSes {
    pub fn for_model_at_rest(model: &Model) -> ObjectTRSes {
        let objects = model.objects.iter().map(|obj| {
            let translation = obj.trans.clone();

            let rotation_quaternion = obj.rot.map(|rotation_matrix| {
                Quaternion::from(rotation_matrix)
                    .normalize()
            });

            let scale = obj.scale.map(|scale| {
                // Bump scalings that are too close to zero.
                let adjust_scale_factor = |s| {
                    // The smallest number on the DS was 2^{-12} (~0.0002).
                    static SMALL: f64 = 0.000_002;
                    if s >= 0.0 && s < SMALL {
                        SMALL
                    } else if s <= 0.0 && s > -SMALL {
                        -SMALL
                    } else {
                        s
                    }
                };

                vec3(
                    adjust_scale_factor(scale.x),
                    adjust_scale_factor(scale.y),
                    adjust_scale_factor(scale.z),
                )
            });

            TRS { translation, rotation_quaternion, scale }
        }).collect::<Vec<TRS>>();

        ObjectTRSes { objects }
    }
}

impl<'a> std::convert::From<&'a TRS> for Matrix4<f64> {
    fn from(trs: &'a TRS) -> Matrix4<f64> {
        let mut m: Matrix4<f64>;
        if let Some(s) = trs.scale {
            m = Matrix4::from_nonuniform_scale(s.x, s.y, s.z);
        } else {
            m = Matrix4::one();
        }
        if let Some(r) = trs.rotation_quaternion {
            m = Matrix4::from(r) * m;
        }
        if let Some(t) = trs.translation {
            m = Matrix4::from_translation(t) * m;
        }
        m
    }
}
