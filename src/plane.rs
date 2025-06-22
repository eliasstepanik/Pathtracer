use crate::algebra::Vec3;
use crate::material::Material;

/// Finite rectangle defined by center-point, normal and half-sizes.
#[derive(Clone)]
pub struct Plane {
    pub point   : Vec3,
    pub normal  : Vec3,
    pub half_w  : f32,
    pub half_h  : f32,
    pub material: Material,
}

impl Plane {
    /// Returns (t, hit_normal, material) or `None` if the ray misses.
    /// Handles both infinite planes (`half_w | half_h = f32::INFINITY`)
    /// and finite rectangles (sizes stored as *half-extents*).
    pub(crate) fn hit(
        &self,
        ro: Vec3,
        rd: Vec3,
    ) -> Option<(f32, Vec3, Material)> {
        // ---------------- intersection with supporting plane
        let denom = self.normal.dot(rd);
        if denom.abs() < 1e-6 {            // ray ‖ plane
            return None;
        }

        let t = self.point.sub(ro).dot(self.normal) / denom;
        if !t.is_finite() || t <= 1e-4 {   // NaN / Inf / behind or self-hit
            return None;
        }

        // ---------------- quick exit for infinite plane
        if !self.half_w.is_finite() || !self.half_h.is_finite() {
            return Some((t, self.normal, self.material));
        }

        // ---------------- rectangle bounds test
        let hit = ro.add(rd.scale(t));

        // Build an orthonormal basis (u,v) in the plane
        let u = self.normal.any_orthonormal().normalize();
        let v = self.normal.cross(u).normalize();

        let d   = hit.sub(self.point);
        let du  = d.dot(u);
        let dv  = d.dot(v);

        if du.abs() <= self.half_w && dv.abs() <= self.half_h {
            Some((t, self.normal, self.material))
        } else {
            None
        }
    }
}