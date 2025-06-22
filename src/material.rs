use crate::algebra::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub color: Vec3,
    pub metallic: f32,
    pub roughness: f32,
    pub ior: f32,
}
