use crate::{algebra::Vec3, material::Material, bvh::BvhNode};

#[derive(Clone)]
pub struct Triangle {
    pub v0: Vec3,
    pub v1: Vec3,
    pub v2: Vec3,
    pub normal: Vec3,
}

pub struct Mesh {
    pub name: String,
    pub triangles: Vec<Triangle>,
    pub bvh: BvhNode,
    pub material: Material,
    pub in_focus: bool,
}

impl Clone for Mesh {
    fn clone(&self) -> Self {
        let triangles = self.triangles.clone();
        let indices: Vec<usize> = (0..triangles.len()).collect();
        let bvh = BvhNode::build(&triangles, &indices);
        Self {
            name: self.name.clone(),
            triangles,
            bvh,
            material: self.material,
            in_focus: self.in_focus,
        }
    }
}

impl Mesh {
    pub fn new(name: String, triangles: Vec<Triangle>, material: Material, in_focus: bool) -> Self {
        let indices: Vec<usize> = (0..triangles.len()).collect();
        let bvh = BvhNode::build(&triangles, &indices);
        Self { name, triangles, bvh, material, in_focus }
    }

    pub fn hit(&self, ro: Vec3, rd: Vec3) -> Option<(f32, Vec3, Material)> {
        let mut result: Option<(f32, Vec3)> = None;
        self.bvh.traverse(&self.triangles, ro, rd, &mut result);
        result.map(|(t, n)| (t, n, self.material))
    }
}

pub fn triangle_intersect(tri: &Triangle, ro: Vec3, rd: Vec3) -> Option<f32> {
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
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => {
                let nums: Vec<f32> = parts.filter_map(|s| s.parse().ok()).collect();
                if nums.len() >= 3 {
                    verts.push(Vec3(nums[0], nums[1], nums[2]));
                }
            }
            Some("f") => {
                let idx: Vec<usize> = parts
                    .filter_map(|s| s.split('/').next().unwrap_or("").parse::<usize>().ok())
                    .collect();
                if idx.len() >= 3 {
                    let first = idx[0] - 1;
                    let mut prev = idx[1] - 1;
                    for &i in &idx[2..] {
                        let v0 = verts[first];
                        let v1 = verts[prev];
                        let v2 = verts[i - 1];
                        tris.push([v0, v1, v2]);
                        prev = i - 1;
                    }
                }
            }
            _ => {}
        }
    }

    tris
}
