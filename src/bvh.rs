use crate::algebra::Vec3;
use crate::mesh::Triangle;

#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn empty() -> Self {
        Self {
            min: Vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY),
            max: Vec3(-f32::INFINITY, -f32::INFINITY, -f32::INFINITY),
        }
    }

    pub fn union(&self, other: &Aabb) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    pub fn union_point(&self, p: Vec3) -> Self {
        Self {
            min: self.min.min(p),
            max: self.max.max(p),
        }
    }

    pub fn from_triangle(tri: &Triangle) -> Self {
        let min = tri.v0.min(tri.v1).min(tri.v2);
        let max = tri.v0.max(tri.v1).max(tri.v2);
        Self { min, max }
    }

    pub fn largest_axis(&self) -> usize {
        let extents = self.max - self.min;
        if extents.0 > extents.1 && extents.0 > extents.2 {
            0
        } else if extents.1 > extents.2 {
            1
        } else {
            2
        }
    }

    pub fn intersects(&self, ro: Vec3, rd: Vec3, mut t_min: f32, mut t_max: f32) -> bool {
        for axis in 0..3 {
            let inv_d = 1.0 / rd.component(axis);
            let mut t0 = (self.min.component(axis) - ro.component(axis)) * inv_d;
            let mut t1 = (self.max.component(axis) - ro.component(axis)) * inv_d;
            if inv_d < 0.0 {
                std::mem::swap(&mut t0, &mut t1);
            }
            t_min = t_min.max(t0);
            t_max = t_max.min(t1);
            if t_max <= t_min {
                return false;
            }
        }
        true
    }
}

pub enum BvhNode {
    Leaf { bbox: Aabb, tris: Vec<usize> },
    Node { bbox: Aabb, left: Box<BvhNode>, right: Box<BvhNode> },
}

impl BvhNode {
    pub fn build(tris: &[Triangle], indices: &[usize]) -> Self {
        let mut bbox = Aabb::empty();
        for &i in indices {
            bbox = bbox.union(&Aabb::from_triangle(&tris[i]));
        }
        if indices.len() <= 4 {
            return BvhNode::Leaf { bbox, tris: indices.to_vec() };
        }
        let mut centroid_bbox = Aabb::empty();
        for &i in indices {
            let c = (tris[i].v0 + tris[i].v1 + tris[i].v2).scale(1.0 / 3.0);
            centroid_bbox = centroid_bbox.union_point(c);
        }
        let axis = centroid_bbox.largest_axis();
        let mut sorted = indices.to_vec();
        sorted.sort_by(|&a, &b| {
            let ca = (tris[a].v0.component(axis) + tris[a].v1.component(axis) + tris[a].v2.component(axis)) / 3.0;
            let cb = (tris[b].v0.component(axis) + tris[b].v1.component(axis) + tris[b].v2.component(axis)) / 3.0;
            ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
        });
        let mid = sorted.len() / 2;
        let left = BvhNode::build(tris, &sorted[..mid]);
        let right = BvhNode::build(tris, &sorted[mid..]);
        BvhNode::Node { bbox, left: Box::new(left), right: Box::new(right) }
    }

    pub fn traverse(&self, tris: &[Triangle], ro: Vec3, rd: Vec3, hit: &mut Option<(f32, Vec3)>) {
        let current_best = hit.map_or(f32::INFINITY, |(t, _)| t);
        let bbox = match self {
            BvhNode::Leaf { bbox, .. } => bbox,
            BvhNode::Node { bbox, .. } => bbox,
        };
        if !bbox.intersects(ro, rd, 0.0, current_best) {
            return;
        }
        match self {
            BvhNode::Leaf { tris: idxs, .. } => {
                for &i in idxs {
                    if let Some(t) = super::mesh::triangle_intersect(&tris[i], ro, rd) {
                        if t > 1e-4 && t < hit.map_or(f32::INFINITY, |(tt, _)| tt) {
                            *hit = Some((t, tris[i].normal));
                        }
                    }
                }
            }
            BvhNode::Node { left, right, .. } => {
                left.traverse(tris, ro, rd, hit);
                right.traverse(tris, ro, rd, hit);
            }
        }
    }
}


