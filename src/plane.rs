use crate::algebra::Vec3;
use crate::material::Material;

/// Finite rectangle defined by center-point, normal and half-sizes.
/// Finite rectangle defined by center-point, and two edge vectors u and v.
#[derive(Clone)]
pub struct Plane {
    pub name    : String,
    pub point   : Vec3,
    pub u       : Vec3, // Vector from center to one edge (encodes direction and half-width)
    pub v       : Vec3, // Vector from center to another edge (encodes direction and half-height)
    pub normal  : Vec3, // Pre-calculated normal (u.cross(v))
    pub material: Material,
}
impl Plane {
    /// Returns (t, hit_normal, material) or `None` if the ray misses.
    pub(crate) fn hit(
        &self,
        ro: Vec3,
        rd: Vec3,
    ) -> Option<(f32, Vec3, Material)> {
        // Intersection with the plane's infinite supporting surface
        let denom = self.normal.dot(rd);
        if denom.abs() < 1e-6 { return None; } // Ray is parallel

        let t = self.point.sub(ro).dot(self.normal) / denom;
        if !t.is_finite() || t <= 1e-4 { return None; }

        // Determine correct normal for two-sided lighting
        let hit_normal = if denom < 0.0 { self.normal } else { self.normal.neg() };

        let hit = ro.add(rd.scale(t));
        let d = hit.sub(self.point); // Vector from plane center to hit point

        // --- NEW, ROBUST BOUNDS CHECK ---
        // Project the vector 'd' onto the plane's basis vectors 'u' and 'v'.
        // If the hit point is inside the rectangle, its coordinates (a, b) in the
        // u,v basis must satisfy |a| <= 1 and |b| <= 1.
        // a = (d . u) / (u . u)
        // b = (d . v) / (v . v)

        let du = d.dot(self.u);
        let u2 = self.u.dot(self.u);

        if du.abs() > u2 { return None; }

        let dv = d.dot(self.v);
        let v2 = self.v.dot(self.v);

        if dv.abs() > v2 { return None; }

        // We have a hit
        Some((t, hit_normal, self.material))
    }
}