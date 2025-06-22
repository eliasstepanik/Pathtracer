use crate::algebra::Vec3;
pub fn reinhard(c:Vec3)->Vec3{ c.scale(1.0/(1.0+c.0)) }

#[inline]
fn aces_film(c: Vec3) -> Vec3 {
    // constants from the paper
    let a = 2.51;
    let b = 0.03;
    let c2 = 2.43;
    let d = 0.59;
    let e = 0.14;

    let map = |x: f32| ((x * (a * x + b)) / (x * (c2 * x + d) + e)).clamp(0.0, 1.0);

    // ACES tone-map, then gamma 2.2 for sRGB
    Vec3(map(c.0).powf(1.0 / 2.2),
         map(c.1).powf(1.0 / 2.2),
         map(c.2).powf(1.0 / 2.2))
}