use cgmath::{vec3, EuclideanSpace, Matrix4, Point3, Rad, Transform, Vector2, Vector3};
use std::default::Default;
use std::f32::consts::PI;

#[derive(Clone)]
pub struct Eye {
    pub position: Point3<f32>,
    pub azimuth: f32,
    pub altitude: f32,
}

impl Eye {
    /// Model-view matrix.
    pub fn model_view(&self) -> Matrix4<f32> {
        let mv = Matrix4::from_angle_x(Rad(-self.altitude))
            * Matrix4::from_angle_y(Rad(-self.azimuth))
            * Matrix4::from_translation(-self.position.to_vec());
        mv
    }

    /// Move in the direction of dv. X = forward, Y = right-side, Z = up.
    pub fn move_by(&mut self, dv: Vector3<f32>) {
        // Treating the eye as if it were inclined neither up nor down,
        // transform the forward/side/up basis in camera space into
        // world space.
        let t = Matrix4::from_angle_y(Rad(self.azimuth));
        let forward = t.transform_vector(vec3(0.0, 0.0, -1.0));
        let side = t.transform_vector(vec3(1.0, 0.0, 0.0));
        let up = t.transform_vector(vec3(0.0, 1.0, 0.0));

        self.position += forward * dv.x + side * dv.y + up * dv.z;
    }

    pub fn free_look(&mut self, dv: Vector2<f32>) {
        self.azimuth -= dv.x;
        self.altitude -= dv.y;

        // Wrap once (expect dv to be small) for azimuth
        static PI_2: f32 = PI * 2.0;
        if self.azimuth >= PI_2 {
            self.azimuth -= PI_2;
        } else if self.azimuth < 0.0 {
            self.azimuth += PI_2;
        }

        // Clamp into allowable altitude range to avoid singularities
        // at the poles.
        let max_alt = 0.499 * PI;
        let min_alt = -max_alt;
        self.altitude = if self.altitude < min_alt {
            min_alt
        } else if self.altitude > max_alt {
            max_alt
        } else {
            self.altitude
        };
    }
}

impl Default for Eye {
    fn default() -> Eye {
        Eye {
            position: Point3::new(0.0, 0.0, 0.0),
            azimuth: 0.0,
            altitude: 0.0,
        }
    }
}
