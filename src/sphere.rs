//! src/sphere.rs
//! -------------
//! Simple UV-sphere with constant radius.

use std::ops::{Add, Sub};
use crate::{algebra::Vec3, material::Material};

#[derive(Clone, Debug)] // MODIFIED: Removed Copy trait
pub struct Sphere {
    pub name     : String, // ADDED
    pub center   : Vec3,
    pub radius   : f32,
    pub material : Material,
    pub in_focus : bool, // ADDED
}

impl Sphere {
    /// Intersect a ray (ro + t·rd).
    /// Returns *closest positive* hit: (t, surface_normal, material).
    pub fn hit(&self,
               ro: Vec3,
               rd: Vec3)
               -> Option<(f32, Vec3, Material)>
    {
        // Analytic quadratic
        let oc   = ro.sub(self.center);
        let a    = rd.dot(rd);
        let b    = 2.0 * oc.dot(rd);
        let c    = oc.dot(oc) - self.radius * self.radius;
        let disc = b*b - 4.0*a*c;
        if disc < 0.0 { return None; }

        let t = (-b - disc.sqrt()) / (2.0 * a);
        if t <= 0.0 { return None; }

        let hit     = ro.add(rd.scale(t));
        let normal  = hit.sub(self.center).scale(1.0 / self.radius); // unit
        Some((t, normal, self.material))
    }
}