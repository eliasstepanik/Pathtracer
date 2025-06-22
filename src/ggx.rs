use crate::algebra::Vec3;
use std::f32::consts::PI;

pub fn reflect(v: Vec3, n: Vec3) -> Vec3 { v.sub(n.scale(2.0*v.dot(n))) }

pub fn fresnel_schlick(cos_theta:f32, f0:Vec3)->Vec3 {
    f0.add(Vec3(1.0,1.0,1.0).sub(f0).scale((1.0-cos_theta).powi(5)))
}
pub fn d_term(nh:f32, a:f32)->f32 {
    let a2=a*a; a2 / (PI*((nh*nh*(a2-1.0)+1.0).powi(2)))
}
pub fn g_term(nv:f32,nl:f32,a:f32)->f32 {
    let g1 = 2.0/(1.0+(1.0+a*a*(1.0-nv*nv)/(nv*nv)).sqrt());
    let g2 = 2.0/(1.0+(1.0+a*a*(1.0-nl*nl)/(nl*nl)).sqrt());
    g1*g2
}