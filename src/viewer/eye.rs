use cgmath::EuclideanSpace;
use cgmath::Matrix4;
use cgmath::PerspectiveFov;
use cgmath::Point3;
use cgmath::Rad;
use cgmath::Transform;
use cgmath::vec3;
use cgmath::Vector2;
use cgmath::Vector3;
use std::f32::consts::PI;

#[derive(Clone)]
pub struct Eye {
    pub position: Point3<f32>,
    pub azimuth: f32,
    pub altitude: f32,
    pub aspect_ratio: f32,
}

impl Eye {
    pub fn model_view(&self) -> Matrix4<f32> {
        let mv =
            Matrix4::from_angle_x(Rad(-self.altitude)) *
            Matrix4::from_angle_y(Rad(-self.azimuth)) *
            Matrix4::from_translation(-self.position.to_vec());
        mv
    }

    pub fn model_view_persp(&self) -> Matrix4<f32> {
        let persp = PerspectiveFov {
            fovy: Rad(1.1),
            aspect: self.aspect_ratio,
            near: 0.01,
            far: 400.0,
        };
        Matrix4::from(persp) * self.model_view()
    }

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
        if self.azimuth >= 2.0 * PI {
            self.azimuth -= 2.0 * PI;
        } else if self.azimuth < 0.0 {
            self.azimuth += 2.0 * PI;
        }

        // Clamp into allowable altitude range to avoid singularities
        // at the poles.
        let max_alt = 0.499 * PI;
        let min_alt = -max_alt;
        self.altitude =
            if self.altitude < min_alt { min_alt }
            else if self.altitude > max_alt { max_alt }
            else { self.altitude };
    }
}
