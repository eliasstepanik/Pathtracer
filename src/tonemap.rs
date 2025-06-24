use crate::algebra::Vec3;
pub fn reinhard(c: Vec3) -> Vec3 {
    Vec3(
        c.0 / (1.0 + c.0),
        c.1 / (1.0 + c.1),
        c.2 / (1.0 + c.2),
    )
}
#[inline]
pub fn aces_film(c: Vec3) -> Vec3 {
    let a = 2.51;
    let b = 0.03;
    let c2 = 2.43;
    let d = 0.59;
    let e = 0.14;

    // Use the map function we created for Vec3
    c.map(|x| ((x * (a * x + b)) / (x * (c2 * x + d) + e)).clamp(0.0, 1.0))
}