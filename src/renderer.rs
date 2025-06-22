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
    let mut total_direct_light = Vec3(0.0, 0.0, 0.0);

    for light in lights {
        let mut light_contrib = Vec3(0.0, 0.0, 0.0);
        let samples = 8;

        for _ in 0..samples {
            // Jitter a point on the emitter rectangle
            let mut rng = rand::thread_rng();
            let lp = light.pos
                .add(light.u.scale(rng.r#gen::<f32>() - 0.5))
                .add(light.v.scale(rng.r#gen::<f32>() - 0.5));
            let lvec = lp.sub(hit);
            let dist2 = lvec.dot(lvec);
            let l = lvec.normalize();

            // Visibility (shadow) test
            let shadow_ro = hit.add(n.scale(1e-4));
            if objects.iter().any(|o|
                o.hit(shadow_ro, l)
                    .map_or(false, |(t, _, _)| t * t < dist2))
            { continue; }

            let n_dot_l = n.dot(l).max(0.0);
            if n_dot_l > 0.0 {
                let h = v.add(l).normalize();

                // --- Start of Correct Cook-Torrance BRDF ---
                let n_dot_v = n.dot(v).max(1e-4);
                let n_dot_h = n.dot(h).max(0.0);
                let v_dot_h = v.dot(h).max(0.0);

                // Fresnel
                let f0 = Vec3(0.04, 0.04, 0.04).scale(1.0 - mat.metallic)
                    .add(mat.color.scale(mat.metallic));
                let f = fresnel_schlick(v_dot_h, f0);

                // Specular BRDF part
                let d = d_term(n_dot_h, mat.roughness);
                let g = g_term(n_dot_v, n_dot_l, mat.roughness);
                let spec_numerator = f.scale(d * g);
                let spec_denominator = 4.0 * n_dot_v * n_dot_l + 1e-6; // add epsilon to avoid division by zero
                let specular = spec_numerator.scale(1.0 / spec_denominator);

                // Diffuse BRDF part (with energy conservation)
                let diffuse_albedo = mat.color.scale(1.0 - mat.metallic);
                let kd = Vec3(1.0, 1.0, 1.0).sub(f); // The portion of light that is not specularly reflected
                let diffuse = diffuse_albedo.mul(kd).scale(1.0 / PI);

                // Add contribution for this sample, scaled by cosine term
                light_contrib = light_contrib.add((diffuse.add(specular)).scale(n_dot_l));
            }
        }

        // Final light contribution is scaled by light's intensity (all channels) and sample count
        let avg_light_contrib = light_contrib.scale(1.0 / samples as f32);
        total_direct_light = total_direct_light.add(avg_light_contrib.mul(light.intensity));
    }

    total_direct_light
}

pub fn render_image_name(w:u32,h:u32,s:u32,ap:f32,f:f32)->String{
    let suf:String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(6).map(char::from).collect();
    format!("renders/render_{w}x{h}_s{s}_ap{ap:.2}_f{f:.1}_{suf}.jpg")
}

pub fn pixel_color(
    x:u32,y:u32,w:u32,h:u32,samples:u32,aspect:f32,scale:f32,
    cam:Vec3,right:Vec3,up:Vec3,forward:Vec3,focus:f32,aperture:f32,
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

            let rd0      = right.scale(u).add(up.scale(v)).add(forward).normalize();

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
    cam: Vec3, right: Vec3, up: Vec3, forward: Vec3,
    aspect: f32, scale: f32, w: u32, h: u32, objs: &[Object]
) -> f32 {
    let mut dists = Vec::new();
    for i in 0..5 {
        for j in 0..5 {
            let px = (w / 2) as f32 + (i as f32 - 2.0);
            let py = (h / 2) as f32 + (j as f32 - 2.0);

            let u = (px / w as f32 - 0.5) * 2.0 * aspect * scale;
            let v = -((py / h as f32 - 0.5) * 2.0 * scale);

            let dir = right.scale(u).add(up.scale(v)).add(forward).normalize();

            if let Some((t, _n, _)) = intersect_closest(cam, dir, objs) {
                dists.push(t);
            }
        }
    }

    if dists.is_empty() {
        10.0
    } else {
        dists.iter().copied().sum::<f32>() / dists.len() as f32
    }
}


// --- MODIFIED: Replaced `trace` with new version supporting IOR ---
pub fn trace(
    ro: Vec3,
    rd: Vec3,
    objs: &[Object],
    lights: &[Light],
    depth: u32,
    glass_bounces: u32,
) -> Vec3 {
    if depth >= MAX_DEPTH {
        return Vec3(0.0, 0.0, 0.0);
    }

    let (t, n, mat) = match intersect_closest(ro, rd, objs) {
        Some(v) => v,
        None => {
            let t = 0.5 * (rd.normalize().1 + 1.0);
            return Vec3(1.0, 1.0, 1.0)
                .scale(1.0 - t)
                .add(Vec3(0.5, 0.7, 1.0).scale(t));
        }
    };
    let hit = ro.add(rd.scale(t));
    let mut rng = rand::thread_rng();

    // --- Glass / Dielectric Branch ---
    if mat.metallic < 0.1 && mat.ior > 0.0 {
        if glass_bounces >= MAX_GLASS_BOUNCES {
            return Vec3(0.0, 0.0, 0.0);
        }

        let cosi = rd.dot(n).clamp(-1.0, 1.0);
        let (etai, etat) = if cosi < 0.0 { (1.0, mat.ior) } else { (mat.ior, 1.0) };
        let hit_normal = if cosi < 0.0 { n } else { n.neg() };

        let r0 = ((etai - etat) / (etai + etat)).powi(2);
        let reflectance = r0 + (1.0 - r0) * (1.0 - cosi.abs()).powi(5);

        if rng.r#gen::<f32>() < reflectance {
            let reflect_dir = reflect(rd, hit_normal);
            let orig = hit.add(hit_normal.scale(1e-4));
            return trace(orig, reflect_dir, objs, lights, depth + 1, glass_bounces + 1);
        } else if let Some(refract_dir) = refract(rd, n, mat.ior) {
            let orig = hit.sub(hit_normal.scale(1e-4));
            return trace(orig, refract_dir, objs, lights, depth + 1, glass_bounces + 1);
        } else { // Total Internal Reflection
            let reflect_dir = reflect(rd, hit_normal);
            let orig = hit.add(hit_normal.scale(1e-4));
            return trace(orig, reflect_dir, objs, lights, depth + 1, glass_bounces + 1);
        }
    }

    // --- Opaque (Diffuse/Glossy) Branch ---
    let direct = lighting(hit, n, rd.neg(), mat, objs, lights);

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

    let indirect_orig = hit.add(n.scale(1e-4));
    let indirect = trace(indirect_orig, hemi_dir, objs, lights, depth + 1, glass_bounces);

    direct.add(indirect.mul(mat.color))
}


fn intersect_closest(ro: Vec3, rd: Vec3, objs: &[Object])
                     -> Option<(f32, Vec3, Material)>
{
    objs.iter()
        .filter_map(|o| o.hit(ro, rd))
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

pub fn refract(dir:Vec3,normal:Vec3,eta:f32)->Option<Vec3>{
    let cosi=-dir.dot(normal).max(-1.0).min(1.0);
    let mut n=normal; let mut ei=1.0; let mut et=eta;
    let mut ci=cosi;
    if ci<0.0 {n=n.neg(); ei=eta; et=1.0; ci=-ci;}
    let eta_r=ei/et; let k=1.0-eta_r*eta_r*(1.0-ci*ci);
    (k>=0.0).then_some(dir.scale(eta_r).add(n.scale(eta_r*ci-k.sqrt())))
}