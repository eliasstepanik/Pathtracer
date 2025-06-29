use crate::{algebra::Vec3, material::Material};

#[derive(Clone)]
pub enum Object {
    Sphere(crate::sphere::Sphere),
    Plane(crate::plane::Plane),
    Mesh(crate::mesh::Mesh),
}

impl Object {
    pub fn hit(
        &self,
        ro: crate::algebra::Vec3,
        rd: crate::algebra::Vec3,
    ) -> Option<(f32, crate::algebra::Vec3, crate::material::Material)> {
        match self {
            Self::Sphere(s) => s.hit(ro, rd),
            Self::Plane(p) => p.hit(ro, rd),
            Self::Mesh(m) => m.hit(ro, rd),
        }
    }

    pub fn is_in_focus(&self) -> bool {
        match self {
            Self::Sphere(s) => s.in_focus,
            Self::Plane(p) => p.in_focus,
            Self::Mesh(m) => m.in_focus,
        }
    }
}
