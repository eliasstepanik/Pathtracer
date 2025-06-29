
use rand::Rng;
use std::f32::consts::PI;
use serde::Deserialize;
use std::ops::{Add, Sub, Mul, Neg};

#[derive(Clone, Copy, Debug)]
#[derive(Default)]
pub struct Vec3(pub f32, pub f32, pub f32);


impl Vec3 {
    #[inline] pub fn scale(self, f: f32) -> Self { Self(self.0*f, self.1*f, self.2*f) }
    #[inline] pub fn dot(self, v: Self) -> f32 { self.0*v.0 + self.1*v.1 + self.2*v.2 }
    #[inline] pub fn cross(self, v: Self) -> Self {
        Self(self.1*v.2-self.2*v.1, self.2*v.0-self.0*v.2, self.0*v.1-self.1*v.0)
    }
    #[inline] pub fn norm(self) -> f32 { self.dot(self).sqrt() }
    #[inline] pub fn normalize(self) -> Self { self.scale(1.0/self.norm()) }

    #[inline] pub fn any_orthonormal(self) -> Vec3 {
        if self.2.abs() < 0.9999999 {
            Vec3(self.1, -self.0, 0.0)
        } else {
            Vec3(0.0, -self.2, self.1)
        }
    }

    #[inline] pub fn lerp(self, v: Self, t: f32) -> Self { self.scale(1.0 - t) + v.scale(t) }

    // --- NEW: map function ---
    /// Applies a function to each component of the vector.
    #[inline]
    pub fn map<F>(self, f: F) -> Self
    where
        F: Fn(f32) -> f32,
    {
        Self(f(self.0), f(self.1), f(self.2))
    }
}

impl Add for Vec3 { type Output = Self; #[inline] fn add(self, v: Self) -> Self { Self(self.0+v.0, self.1+v.1, self.2+v.2) } }
impl Sub for Vec3 { type Output = Self; #[inline] fn sub(self, v: Self) -> Self { Self(self.0-v.0, self.1-v.1, self.2-v.2) } }
impl Mul for Vec3 { type Output = Self; #[inline] fn mul(self, v: Self) -> Self { Self(self.0*v.0, self.1*v.1, self.2*v.2) } } // Element-wise
impl Mul<f32> for Vec3 { type Output = Self; #[inline] fn mul(self, f: f32) -> Self { self.scale(f) } }
impl Neg for Vec3 { type Output = Self; #[inline] fn neg(self) -> Self { Self(-self.0,-self.1,-self.2) } }


impl From<[f32; 3]> for Vec3 {
    fn from(a: [f32; 3]) -> Self { Vec3(a[0], a[1], a[2]) }
}

impl From<Vec3> for [f32; 3] {
    fn from(v: Vec3) -> Self { [v.0, v.1, v.2] }
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