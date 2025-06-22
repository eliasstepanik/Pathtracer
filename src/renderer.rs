use crate::{
    algebra::{sample_disk, Vec3},
    ggx::{d_term, g_term, fresnel_schlick, reflect},
    material::Material,
    light::Light,
    object::Object,
    tonemap,
};
use image::Rgb;
use rand::Rng;
use rayon::prelude::*;
use std::f32::consts::PI;

const MAX_DEPTH: u32 = 12;
const MAX_GLASS_BOUNCES: u32 = 8;

/// direct lighting from a *list* of (rectangular) area lights
fn lighting(
    hit: Vec3, n: Vec3, v: Vec3,
    mat: Material,
    objects: &[Object],
    lights : &[Light],
) -> Vec3 {
    let mut diff = Vec3(0.0, 0.0, 0.0);
    let mut spec = Vec3(0.0, 0.0, 0.0);

    for light in lights {
        let samples = 8;                     // stratified samples per light
        for _ in 0..samples {
            // jitter a point on the emitter rectangle
            let mut rng   = rand::thread_rng();
            let lp        = light.pos
                .add(light.u.scale(rng.r#gen::<f32>() - 0.5))
                .add(light.v.scale(rng.r#gen::<f32>() - 0.5));
            let lvec      = lp.sub(hit);
            let dist2     = lvec.dot(lvec);
            let l         = lvec.normalize();

            // visibility (shadow) test
            let shadow_ro = hit.add(n.scale(0.001));
            if objects.iter().any(|o|
                o.hit(shadow_ro, l)
                    .map_or(false, |(t, _, _)| t * t < dist2))
            { continue; }

            // Lambert
            let n_dot_l = n.dot(l).max(0.0);
            diff = diff.add(mat.color.scale(n_dot_l));

            // GGX micro-facet specular
            let h         = l.add(v).normalize();
            let n_dot_v   = n.dot(v).max(1e-4);
            let n_dot_l2  = n_dot_l.max(1e-4);
            let n_dot_h   = n.dot(h).max(0.0);
            let v_dot_h   = v.dot(h).max(0.0);
            let f0        = Vec3(0.04, 0.04, 0.04)
                .add(mat.color.scale(mat.metallic));
            let f         = fresnel_schlick(v_dot_h, f0);
            let d         = d_term(n_dot_h, mat.roughness);
            let g         = g_term(n_dot_v, n_dot_l2, mat.roughness);
            let spec_c    = f.scale(d * g / (4.0 * n_dot_v * n_dot_l2));

            spec = spec.add(spec_c.scale(n_dot_l));
        }
        // scale by light power (divide by sample count)
        diff = diff.scale(light.intensity.0 / samples as f32);
        spec = spec.scale(light.intensity.0 / samples as f32);
    }
    diff.add(spec)
}

pub fn render_image_name(w:u32,h:u32,s:u32,ap:f32,f:f32)->String{
    let suf:String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(6).map(char::from).collect();
    format!("renders/render_{w}x{h}_s{s}_ap{ap:.2}_f{f:.1}_{suf}.jpg")
}

pub fn pixel_color(
    x:u32,y:u32,w:u32,h:u32,samples:u32,aspect:f32,scale:f32,
    cam:Vec3,right:Vec3,up:Vec3,focus:f32,aperture:f32,
    objs:&[Object],lights:&[Light],rng:&mut impl Rng)->[u8;3]
{
    let sqrt_s = (samples as f32).sqrt() as u32;
    let mut col = Vec3(0.0,0.0,0.0);

    for i in 0..sqrt_s {
        for j in 0..sqrt_s {
            let jx = (i as f32 + rng.r#gen::<f32>()) / sqrt_s as f32;
            let jy = (j as f32 + rng.r#gen::<f32>()) / sqrt_s as f32;
            let u  = ((x as f32 + jx)/w as f32 -0.5)*2.0*aspect*scale;
            let v  = -((y as f32 + jy)/h as f32 -0.5)*2.0*scale;

            let rd0      = Vec3(u,v,1.0).normalize();
            let (dx,dy)  = sample_disk(rng.r#gen::<f32>()*aperture);
            let focal_pt = cam.add(rd0.scale(focus));
            let origin   = cam.add(right.scale(dx)).add(up.scale(dy));
            let rd       = focal_pt.sub(origin).normalize();

            col = col.add(trace(origin,rd,objs,lights,0,0));
        }
    }
    col = tonemap::reinhard(col.scale(1.0/samples as f32));
    [
        (col.0*255.0).min(255.0) as u8,
        (col.1*255.0).min(255.0) as u8,
        (col.2*255.0).min(255.0) as u8
    ]
}

pub fn autofocus(
    cam:Vec3,aspect:f32,scale:f32,w:u32,h:u32,objs:&[Object])->f32
{
    let mut dists=Vec::new();
    for i in 0..5{
        for j in 0..5{
            let u=((w/2+i-2)as f32)/w as f32*2.0-1.0;
            let v=((h/2+j-2)as f32)/h as f32*2.0-1.0;
            let dir=Vec3(u*aspect*scale,-v*scale,1.0).normalize();
            if let Some((t,n,_)) = intersect_closest(cam,dir,objs){
                dists.push(cam.add(dir.scale(t)).sub(n.scale(0.1)).sub(cam).norm());
            }
        }
    }
    dists.iter().copied().sum::<f32>()/dists.len().max(1) as f32
}

// ─────────────────────────────────────────────────────────── trace & helpers
pub fn trace(
    ro: Vec3,
    rd: Vec3,
    objs: &[Object],
    lights: &[Light],
    depth: u32,
    glass: u32,
) -> Vec3 {
    if depth >= MAX_DEPTH || glass >= MAX_GLASS_BOUNCES {
        return Vec3(0.0, 0.0, 0.0);
    }

    // closest hit
    let (t, n, mat) = match intersect_closest(ro, rd, objs) {
        Some(v) => v,
        None => {
            let t = 0.5 * (rd.1 + 1.0);
            return Vec3(1.0, 1.0, 1.0)
                .scale(1.0 - t)
                .add(Vec3(0.5, 0.7, 1.0).scale(t));
        }
    };
    let hit = ro.add(rd.scale(t));

    /* …  glass branch identical … */

    /* diffuse / glossy branch */
    let direct = lighting(hit, n, rd.neg(), mat, objs, lights);

    // cosine-weighted hemisphere
    let mut rng = rand::thread_rng();
    let w = n;
    let u = if w.0.abs() > 0.1 {
        w.cross(Vec3(0.0, 1.0, 0.0)).normalize()
    } else {
        w.cross(Vec3(1.0, 0.0, 0.0)).normalize()
    };
    let v = w.cross(u);
    let r1: f32 = rng.r#gen();
    let r2: f32 = rng.r#gen();
    let phi = 2.0 * PI * r1;
    let hemi_dir = u.scale(phi.cos() * r2.sqrt())
        .add(v.scale(phi.sin() * r2.sqrt()))
        .add(w.scale((1.0 - r2).sqrt()))
        .normalize();

    let indirect = trace(hit.add(n.scale(0.001)), hemi_dir, objs, lights, depth + 1, glass);
    direct.add(indirect.scale(mat.color.0))
}


fn intersect_closest(ro: Vec3, rd: Vec3, objs: &[Object])
                     -> Option<(f32, Vec3, Material)>
{
    objs.iter()
        .filter_map(|o| o.hit(ro, rd))
        // `partial_cmp` may return `None` for NaN values which causes a panic
        // when unwrapped. `total_cmp` provides a total ordering for `f32` and
        // therefore guarantees an `Ordering` even in presence of NaN.
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

/// Snell refraction
pub fn refract(dir:Vec3,normal:Vec3,eta:f32)->Option<Vec3>{
    let cosi=-dir.dot(normal).max(-1.0).min(1.0);
    let mut n=normal; let mut ei=1.0; let mut et=eta;
    let mut ci=cosi;
    if ci<0.0 {n=n.neg(); ei=eta; et=1.0; ci=-ci;}
    let eta_r=ei/et; let k=1.0-eta_r*eta_r*(1.0-ci*ci);
    (k>=0.0).then_some(dir.scale(eta_r).add(n.scale(eta_r*ci-k.sqrt())))
}
