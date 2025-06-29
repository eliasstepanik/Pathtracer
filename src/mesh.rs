use crate::{algebra::Vec3, material::Material};

#[derive(Clone)]
pub struct Triangle {
    pub v0: Vec3,
    pub v1: Vec3,
    pub v2: Vec3,
    pub normal: Vec3,
}

#[derive(Clone)]
pub struct Mesh {
    pub name: String,
    pub triangles: Vec<Triangle>,
    pub material: Material,
    pub in_focus: bool,
}

impl Mesh {
    pub fn hit(&self, ro: Vec3, rd: Vec3) -> Option<(f32, Vec3, Material)> {
        let mut closest_t = f32::INFINITY;
        let mut hit_normal = Vec3(0.0, 0.0, 0.0);
        for tri in &self.triangles {
            if let Some(t) = triangle_intersect(tri, ro, rd) {
                if t > 1e-4 && t < closest_t {
                    closest_t = t;
                    hit_normal = tri.normal;
                }
            }
        }
        if closest_t < f32::INFINITY {
            Some((closest_t, hit_normal, self.material))
        } else {
            None
        }
    }
}

fn triangle_intersect(tri: &Triangle, ro: Vec3, rd: Vec3) -> Option<f32> {
    let e1 = tri.v1 - tri.v0;
    let e2 = tri.v2 - tri.v0;
    let p = rd.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-8 {
        return None;
    }
    let inv_det = 1.0 / det;
    let tvec = ro - tri.v0;
    let u = tvec.dot(p) * inv_det;
    if u < 0.0 || u > 1.0 {
        return None;
    }
    let q = tvec.cross(e1);
    let v = rd.dot(q) * inv_det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv_det;
    (t > 0.0).then_some(t)
}

pub fn load_obj(path: &str) -> Vec<[Vec3; 3]> {
    let data = std::fs::read_to_string(path).expect("obj file");
    let mut verts = Vec::new();
    let mut tris = Vec::new();
    for line in data.lines() {
        if let Some(rest) = line.strip_prefix('v') {
            if let Some(rest) = rest.strip_prefix(' ') {
                let nums: Vec<f32> = rest
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if nums.len() >= 3 {
                    verts.push(Vec3(nums[0], nums[1], nums[2]));
                }
            }
        } else if let Some(rest) = line.strip_prefix('f') {
            if let Some(rest) = rest.strip_prefix(' ') {
                let idx: Vec<usize> = rest
                    .split_whitespace()
                    .filter_map(|s| s.split('/').next().unwrap_or("").parse::<usize>().ok())
                    .collect();
                if idx.len() >= 3 {
                    let v0 = verts[idx[0] - 1];
                    let v1 = verts[idx[1] - 1];
                    let v2 = verts[idx[2] - 1];
                    tris.push([v0, v1, v2]);
                }
            }
        }
    }
    tris
}
