use crate::{algebra::Vec3, material::Material};

#[derive(Clone)]
pub struct Triangle {
    pub v0: Vec3,
    pub v1: Vec3,
    pub v2: Vec3,
    pub material: Material,
}

impl Triangle {
    pub fn normal(&self) -> Vec3 {
        (self.v1 - self.v0).cross(self.v2 - self.v0).normalize()
    }

    pub fn hit(&self, ro: Vec3, rd: Vec3) -> Option<(f32, Vec3, Material)> {
        let edge1 = self.v1 - self.v0;
        let edge2 = self.v2 - self.v0;
        let h = rd.cross(edge2);
        let a = edge1.dot(h);
        if a.abs() < 1e-6 { return None; }
        let f = 1.0 / a;
        let s = ro - self.v0;
        let u = f * s.dot(h);
        if !(0.0..=1.0).contains(&u) { return None; }
        let q = s.cross(edge1);
        let v = f * rd.dot(q);
        if v < 0.0 || u + v > 1.0 { return None; }
        let t = f * edge2.dot(q);
        if t > 1e-4 { Some((t, self.normal(), self.material)) } else { None }
    }
}

#[derive(Clone)]
pub struct Mesh {
    pub name: String,
    pub triangles: Vec<Triangle>,
    pub in_focus: bool,
}

impl Mesh {
    pub fn hit(&self, ro: Vec3, rd: Vec3) -> Option<(f32, Vec3, Material)> {
        self.triangles
            .iter()
            .filter_map(|tri| tri.hit(ro, rd))
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
    }
}
