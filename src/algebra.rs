use rand::Rng;
use std::f32::consts::PI;
use serde::Deserialize;

#[derive(Clone, Copy, Debug)]
pub struct Vec3(pub f32, pub f32, pub f32);


impl Vec3 {
    pub fn add(self, v: Self) -> Self { Self(self.0+v.0, self.1+v.1, self.2+v.2) }
    pub fn sub(self, v: Self) -> Self { Self(self.0-v.0, self.1-v.1, self.2-v.2) }
    pub fn scale(self, f: f32) -> Self { Self(self.0*f, self.1*f, self.2*f) }
    pub fn dot(self, v: Self) -> f32 { self.0*v.0 + self.1*v.1 + self.2*v.2 }
    pub fn cross(self, v: Self) -> Self {
        Self(self.1*v.2-self.2*v.1, self.2*v.0-self.0*v.2, self.0*v.1-self.1*v.0)
    }
    pub fn norm(self) -> f32 { self.dot(self).sqrt() }
    pub fn normalize(self) -> Self { self.scale(1.0/self.norm()) }
    pub fn neg(self) -> Self { Self(-self.0,-self.1,-self.2) }

    pub fn any_orthonormal(self) -> Vec3 {
        // Pick the smallest‐magnitude component to avoid near-zero cross products
        if self.0.abs() < self.1.abs() && self.0.abs() < self.2.abs() {
            // x is smallest → use (0, -z,  y)
            Vec3(0.0, -self.2,  self.1)
        } else if self.1.abs() < self.2.abs() {
            // y is smallest → use (-z, 0,  x)
            Vec3(-self.2, 0.0,  self.0)
        } else {
            // z is smallest → use ( y, -x, 0)
            Vec3(self.1, -self.0, 0.0)
        }
    }
}


impl From<[f32; 3]> for Vec3 {
    fn from(a: [f32; 3]) -> Self { Vec3(a[0], a[1], a[2]) }
}

/* Custom helper so Serde turns a JSON array into Vec3 */
pub fn vec3_from_array<'de, D>(d: D) -> Result<Vec3, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let arr = <[f32; 3]>::deserialize(d)?;
    Ok(arr.into())
}


/// concentric-disk sample (for depth-of-field)
pub fn sample_disk(r: f32) -> (f32,f32) {
    let mut rng = rand::thread_rng();
    let s = rng.r#gen::<f32>();
    let t = rng.r#gen::<f32>();
    let ang = 2.0*PI*s; let rad = r*t.sqrt();
    (rad*ang.cos(), rad*ang.sin())
}