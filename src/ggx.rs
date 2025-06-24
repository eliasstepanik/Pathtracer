use crate::algebra::Vec3;
use std::f32::consts::PI;
use rand::Rng;

pub fn reflect(v: Vec3, n: Vec3) -> Vec3 { v - n * 2.0 * v.dot(n) }

pub fn fresnel_schlick(cos_theta:f32, f0:Vec3)->Vec3 {
    f0 + (Vec3(1.0,1.0,1.0) - f0) * (1.0-cos_theta).powi(5)
}
pub fn d_term(nh:f32, a:f32)->f32 {
    let a2=a*a; a2 / (PI*((nh*nh*(a2-1.0)+1.0).powi(2)))
}
pub fn g_term(nv:f32,nl:f32,a:f32)->f32 {
    let k = a*a/2.0; // Approximation for G smith correlated
    let g1 = nv/(nv*(1.0-k)+k);
    let g2 = nl/(nl*(1.0-k)+k);
    g1*g2
}

pub fn sample_ggx_h(n: Vec3, roughness: f32, rng: &mut impl Rng) -> Vec3 {
    let a = roughness * roughness;
    let a2 = a * a;

    let r1: f32 = rng.r#gen();
    let r2: f32 = rng.r#gen();

    let phi = 2.0 * PI * r1;
    let cos_theta = ((1.0 - r2) / (1.0 + (a2 - 1.0) * r2)).sqrt();
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();

    // vector in tangent space
    let h_tangent = Vec3(
        phi.cos() * sin_theta,
        phi.sin() * sin_theta,
        cos_theta,
    );

    // create orthonormal basis around normal n
    let w = n;
    let u = n.any_orthonormal().normalize();
    let v = w.cross(u);

    // transform from tangent space to world space
    u * h_tangent.0 + v * h_tangent.1 + w * h_tangent.2
}