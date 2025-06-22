use crate::algebra::Vec3;

#[derive(Clone, Copy)]
pub struct Light {
    pub pos: Vec3,
    pub u:   Vec3,
    pub v:   Vec3,
    pub intensity: Vec3,
}