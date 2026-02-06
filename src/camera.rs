use crate::math::Vec3; 

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniforms {
    pub origin: [f32; 3],
    _pad1: f32,
    pub u: [f32; 3],
    _pad2: f32,
    pub v: [f32; 3],
    _pad3: f32,
    pub w: [f32; 3],
    _pad4: f32,
}

pub struct Camera {
    pub lookfrom: Vec3,
    pub lookat: Vec3,
    pub vup: Vec3,
    pub vfov: f32, 
}

impl Camera {
    pub fn new(lookfrom: Vec3, lookat: Vec3, vup: Vec3, vfov: f32) -> Self {
        Self {
            lookfrom,
            lookat,
            vup,
            vfov,
        }
    }

    pub fn get_uniforms(&self) -> CameraUniforms {
        let theta = self.vfov.to_radians();
        let h = (theta / 2.0).tan();

       
        let w = (self.lookfrom - self.lookat).normalized();
        let u = self.vup.cross(&w).normalized(); 
        let v = w.cross(&u);

        let u_scaled = u * h;
        let v_scaled = v * h;
        let w_forward = -w;

        
        CameraUniforms {
            origin: [self.lookfrom.x(), self.lookfrom.y(), self.lookfrom.z()],
            _pad1: 0.0,
            u: [u_scaled.x(), u_scaled.y(), u_scaled.z()],
            _pad2: 0.0,
            v: [v_scaled.x(), v_scaled.y(), v_scaled.z()],
            _pad3: 0.0,
            w: [w_forward.x(), w_forward.y(), w_forward.z()],
            _pad4: 0.0,
        }
    }

    pub fn zoom(&mut self, delta: f32) {
        self.vfov -= delta * 10.0;
        if self.vfov < 1.0 { self.vfov = 1.0; }
        if self.vfov > 179.0 { self.vfov = 179.0; }
    }

    pub fn move_along_w(&mut self, delta: f32) {
        let w = (self.lookat - self.lookfrom).normalized();
        let move_vec = w * delta * 5.0;
        self.lookfrom += move_vec;
        self.lookat += move_vec;
    }

    pub fn move_along_u(&mut self, delta: f32) {
        let w = (self.lookfrom - self.lookat).normalized();
        let u = self.vup.cross(&w).normalized();
        let move_vec = u * delta * 5.0;
        self.lookfrom += move_vec;
        self.lookat += move_vec;
    }

    pub fn rotate(&mut self, dx: f32, dy: f32) {
        let mut forward = self.lookat - self.lookfrom;
        
        let cos_yaw = dx.cos();
        let sin_yaw = dx.sin();
        let new_x = forward.x() * cos_yaw - forward.z() * sin_yaw;
        let new_z = forward.x() * sin_yaw + forward.z() * cos_yaw;
        forward = Vec3::new(new_x, forward.y(), new_z);

        let w = -forward.normalized();
        let u = self.vup.cross(&w).normalized();
        
        let new_y = forward.y() + dy;
        forward = Vec3::new(forward.x(), new_y, forward.z());

        self.lookat = self.lookfrom + forward;
    }
}
