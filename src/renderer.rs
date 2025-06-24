use crate::{
    algebra::{sample_disk, Vec3},
    ggx::{d_term, g_term, fresnel_schlick, reflect, sample_ggx_h},
    material::Material,
    light::Light,
    object::Object,
    tonemap,
};
use image::Rgb;
use rand::Rng;
use rayon::prelude::*;
use std::f32::consts::PI;
use std::ops::Mul;

const MAX_DEPTH: u32 = 5;
const RUSSIAN_ROULETTE_DEPTH: u32 = 2;


fn direct_light_sample(
    hit: Vec3, n: Vec3, v: Vec3,
    mat: Material,
    objs: &[Object],
    lights: &[Light],
    rng: &mut impl Rng,
) -> Vec3 {
    let mut total_direct_light = Vec3(0.0, 0.0, 0.0);

    // --- NEW: Define number of shadow rays per intersection ---
    const SHADOW_SAMPLES: u32 = 4; // Increase this for smoother shadows at the cost of performance. 4 is a good balance.

    for light in lights {
        let mut light_contrib = Vec3(0.0, 0.0, 0.0);

        // --- NEW: Loop to cast multiple shadow rays ---
        for _ in 0..SHADOW_SAMPLES {
            // Sample a point on the light source
            let lp = light.pos
                + light.u * (rng.gen::<f32>() - 0.5)
                + light.v * (rng.gen::<f32>() - 0.5);
            let lvec = lp - hit;
            let dist2 = lvec.dot(lvec);
            let l = lvec.normalize();

            // Check for visibility (shadow ray)
            let shadow_ro = hit + l * 1e-4;
            if objs.iter().any(|o| o.hit(shadow_ro, l).map_or(false, |(t, _, _)| t * t < dist2 * 0.999))
            { continue; }

            let n_dot_l = n.dot(l).max(0.0);
            if n_dot_l > 0.0 {
                let light_area = light.u.cross(light.v).norm();
                let light_normal = light.u.cross(light.v).normalize();
                let cos_theta_light = (-l).dot(light_normal).max(0.0);

                if cos_theta_light > 0.0 {
                    let falloff = cos_theta_light / dist2;

                    let h = (v + l).normalize();
                    let n_dot_v = n.dot(v).max(1e-4);
                    let n_dot_h = n.dot(h).max(0.0);
                    let v_dot_h = v.dot(h).max(0.0);

                    let f0 = Vec3(0.04, 0.04, 0.04) * (1.0 - mat.metallic) + mat.color * mat.metallic;
                    let f = fresnel_schlick(v_dot_h, f0);
                    let d = d_term(n_dot_h, mat.roughness);
                    let g = g_term(n_dot_v, n_dot_l, mat.roughness);

                    let spec_numerator = f * d * g;
                    let spec_denominator = 4.0 * n_dot_v * n_dot_l;
                    let specular_brdf = spec_numerator * (1.0 / (spec_denominator + 1e-6));

                    let diffuse_color = mat.color * (1.0 - mat.metallic);
                    let k_d = Vec3(1.0, 1.0, 1.0) - f;
                    let diffuse_brdf = diffuse_color.mul(k_d) * (1.0 / PI);

                    let radiance = (diffuse_brdf + specular_brdf) * n_dot_l;
                    light_contrib = light_contrib + radiance.mul(light.intensity).scale(light_area * falloff);
                }
            }
        }
        // Average the contribution from all shadow samples
        total_direct_light = total_direct_light + light_contrib.scale(1.0 / SHADOW_SAMPLES as f32);
    }
    total_direct_light
}


// ... lighting, render_image_name, pixel_color, autofocus functions remain the same as the previous answer ...

fn lighting(
    hit: Vec3, n: Vec3, v: Vec3,
    mat: Material,
    objects: &[Object],
    lights : &[Light],
    rng: &mut impl Rng,
) -> Vec3 {
    let mut total_direct_light = Vec3(0.0, 0.0, 0.0);

    for light in lights {
        let mut light_contrib = Vec3(0.0, 0.0, 0.0);
        let samples = 1; // Direct light is expensive, we can rely on pixel samples

        for _ in 0..samples {
            let lp = light.pos
                + light.u * (rng.gen::<f32>() - 0.5)
                + light.v * (rng.gen::<f32>() - 0.5);
            let lvec = lp - hit;
            let dist2 = lvec.dot(lvec);
            let l = lvec.normalize();

            let shadow_ro = hit + n * 1e-4;
            if objects.iter().any(|o|
                o.hit(shadow_ro, l)
                    .map_or(false, |(t, _, _)| t * t < dist2))
            { continue; }

            let n_dot_l = n.dot(l).max(0.0);
            if n_dot_l > 0.0 {
                // --- NEW: Area Light Attenuation ---
                // For area lights, we must account for the solid angle they occupy.
                // This term scales the light based on its area and distance.
                let light_area = light.u.cross(light.v).norm();
                let light_normal = light.u.cross(light.v).normalize();
                let cos_theta_light = (-l).dot(light_normal).max(0.0);
                let falloff = cos_theta_light / (dist2 + 1e-4); // +1 for no light at source

                let h = (v + l).normalize();
                let n_dot_v = n.dot(v).max(1e-4);
                let n_dot_h = n.dot(h).max(0.0);
                let v_dot_h = v.dot(h).max(0.0);

                let f0 = Vec3(0.04, 0.04, 0.04) * (1.0 - mat.metallic) + mat.color * mat.metallic;
                let f = fresnel_schlick(v_dot_h, f0);
                let d = d_term(n_dot_h, mat.roughness);
                let g = g_term(n_dot_v, n_dot_l, mat.roughness);

                let spec_numerator = f * d * g;
                let spec_denominator = 4.0 * n_dot_v * n_dot_l;
                let specular_brdf = spec_numerator * (1.0 / (spec_denominator + 1e-6));

                let diffuse_color = mat.color * (1.0 - mat.metallic);
                let k_d = Vec3(1.0, 1.0, 1.0) - f;
                let diffuse_brdf = diffuse_color.mul(k_d) * (1.0 / PI);

                // Combine and scale by light properties
                let radiance = (diffuse_brdf + specular_brdf) * n_dot_l;
                light_contrib = light_contrib + radiance.mul(light.intensity).scale(light_area * falloff);
            }
        }
        total_direct_light = total_direct_light + light_contrib * (1.0 / samples as f32);
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
    let mut col = Vec3(0.0,0.0,0.0);
    for _ in 0..samples {
        let jx = rng.gen::<f32>();
        let jy = rng.gen::<f32>();
        let u  = ((x as f32 + jx)/w as f32 -0.5)*2.0*aspect*scale;
        let v  = -((y as f32 + jy)/h as f32 -0.5)*2.0*scale;
        let rd0 = (right*u + up*v + forward).normalize();
        let (dx,dy)  = sample_disk(aperture);
        let focal_pt = cam + rd0*focus;
        let origin   = cam + right*dx + up*dy;
        let rd       = (focal_pt - origin).normalize();

        // Initial call to trace starts with no medium.
        col = col + trace(origin, rd, objs, lights, 0, rng, None);
    }

    let avg_col = col * (1.0/samples as f32);
    let tonemapped_col = tonemap::aces_film(avg_col);

    [
        (tonemapped_col.0.powf(1.0/2.2)*255.0).min(255.0) as u8,
        (tonemapped_col.1.powf(1.0/2.2)*255.0).min(255.0) as u8,
        (tonemapped_col.2.powf(1.0/2.2)*255.0).min(255.0) as u8
    ]
}

fn sample_phase_function(g: f32, rng: &mut impl Rng) -> f32 {
    if g.abs() < 1e-3 {
        return 1.0 - 2.0 * rng.gen::<f32>();
    }
    let r: f32 = rng.gen();
    let g2 = g * g;
    let term = (1.0 - g2) / (1.0 - g + 2.0 * g * r);
    (1.0 + g2 - term * term) / (2.0 * g)
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

            let dir = (right.scale(u) + up.scale(v) + forward).normalize();

            if let Some((t, _n, _)) = intersect_closest(cam, dir, objs) {
                dists.push(t);
            }
        }
    }

    if dists.is_empty() { 10.0 } else { dists.iter().copied().sum::<f32>() / dists.len() as f32 }
}


pub fn trace(
    ro: Vec3,
    rd: Vec3,
    objs: &[Object],
    lights: &[Light],
    depth: u32,
    rng: &mut impl Rng,
    mut current_media: Option<Material>,
) -> Vec3 {
    if depth >= MAX_DEPTH { return Vec3(0.0, 0.0, 0.0); }

    // --- 1. Find the next potential surface interaction ---
    let surface_hit = intersect_closest(ro, rd, objs);
    let t_surface = surface_hit.as_ref().map_or(f32::INFINITY, |(t, _, _)| *t);

    // --- 2. Ray March through the current medium (if any) ---
    let mut t_media = f32::INFINITY;
    let mut absorption = Vec3(1.0, 1.0, 1.0);
    if let Some(media) = current_media {
        if media.volume_density > 0.0 {
            let sample_dist = -rng.gen::<f32>().ln() / media.volume_density;
            t_media = sample_dist;

            // Calculate absorption over the distance travelled to the event
            let absorption_coeff = media.color.map(|c| (1.0 - c).max(0.0) * media.volume_density);
            absorption = (-absorption_coeff * t_media.min(t_surface)).map(f32::exp);
        }
    }

    // --- 3. Decide what event happens first: surface hit or media scatter ---

    // A. Media scattering event happens first
    if t_media < t_surface {
        let hit_point = ro + rd * t_media;

        // Add direct lighting at the scatter point (for god rays)
        let direct_light = direct_light_sample(hit_point, Vec3(0.0,1.0,0.0), -rd, current_media.unwrap(), objs, lights, rng);

        // Scatter the ray using the phase function
        let w = rd;
        let u = w.any_orthonormal().normalize();
        let v_cross = w.cross(u);
        let cos_theta = sample_phase_function(current_media.unwrap().volume_anisotropy, rng);
        let sin_theta = (1.0 - cos_theta*cos_theta).sqrt();
        let phi = 2.0 * PI * rng.gen::<f32>();
        let next_dir = (u * phi.cos() * sin_theta + v_cross * phi.sin() * sin_theta + w * cos_theta).normalize();

        // Recurse from the scatter point, staying in the same medium
        return (direct_light + trace(hit_point, next_dir, objs, lights, depth + 1, rng, current_media)).mul(absorption);
    }

    // B. Surface hit event happens first (or no medium)
    let (t, n, mut mat) = match surface_hit {
        Some(v) => v,
        None => return Vec3(0.0, 0.0, 0.0).mul(absorption) // Hit sky, attenuated by any medium we passed through
    };

    let hit = ro + rd * t;
    let v = -rd;

    mat.metallic = mat.metallic.clamp(0.0, 1.0);
    mat.roughness = mat.roughness.clamp(0.01, 1.0);

    // Determine the medium for the *next* ray bounce
    let next_media = if mat.volume_density > 0.0 {
        if v.dot(n) > 0.0 { Some(mat) } else { None } // Entering vs. Exiting
    } else {
        current_media
    };

    if mat.ior > 1.0 && mat.metallic < 0.1 { // Glass Surface
        // Standard glass logic, but the recursive call passes the `next_media`
        let cosi = v.dot(n).clamp(-1.0, 1.0);
        let (etai, etat) = if cosi > 0.0 { (1.0, mat.ior) } else { (mat.ior, 1.0) };
        let hit_normal = if cosi > 0.0 { n } else { -n };
        let r0 = ((etai - etat) / (etai + etat)).powi(2);
        let reflectance = r0 + (1.0 - r0) * (1.0 - cosi.abs()).powi(5);

        let next_dir = if rng.gen::<f32>() < reflectance { reflect(-v, hit_normal) }
        else if let Some(refract_dir) = refract(-v, hit_normal, etai / etat) { refract_dir }
        else { reflect(-v, hit_normal) };

        return trace(hit + next_dir * 1e-4, next_dir, objs, lights, depth + 1, rng, next_media).mul(absorption);
    }

    // Opaque Surface
    let direct_light = direct_light_sample(hit, n, v, mat, objs, lights, rng);
    let mut indirect_light = Vec3(0.0, 0.0, 0.0);

    // Russian Roulette, etc.
    let p = mat.color.0.max(mat.color.1).max(mat.color.2);
    if depth < RUSSIAN_ROULETTE_DEPTH || rng.gen::<f32>() < p {
        let (next_dir, brdf) = if rng.gen::<f32>() < (1.0 - mat.metallic) { // Diffuse
            let w = n;
            let u = w.any_orthonormal().normalize();
            let v_cross = w.cross(u);
            let phi = 2.0 * PI * rng.gen::<f32>();
            let r2: f32 = rng.gen();
            ( (u * phi.cos() * r2.sqrt() + v_cross * phi.sin() * r2.sqrt() + w * (1.0 - r2).sqrt()).normalize(),
              mat.color * (1.0 / PI) )
        } else { // Specular
            let h = sample_ggx_h(n, mat.roughness, rng);
            ( reflect(-v, h),
              Vec3(1.0,1.0,1.0) ) // Specular BRDF handled by fresnel in throughput
        };

        if next_dir.dot(n) > 0.0 {
            let incoming = trace(hit + next_dir * 1e-4, next_dir, objs, lights, depth + 1, rng, next_media);
            indirect_light = incoming.mul(brdf).scale(next_dir.dot(n));
            if depth >= RUSSIAN_ROULETTE_DEPTH {
                indirect_light = indirect_light.scale(1.0 / p);
            }
        }
    }

    return (direct_light + indirect_light).mul(absorption);
}


fn intersect_closest(ro: Vec3, rd: Vec3, objs: &[Object])
                     -> Option<(f32, Vec3, Material)>
{
    objs.iter()
        .filter_map(|o| o.hit(ro, rd))
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

pub fn refract(v: Vec3, n: Vec3, eta_ratio: f32) -> Option<Vec3> {
    let cos_theta = (-v).dot(n).min(1.0);
    let r_out_perp = (v + n * cos_theta) * eta_ratio;
    let r_out_parallel = n * -(1.0 - r_out_perp.dot(r_out_perp)).abs().sqrt();
    (r_out_perp.dot(r_out_perp) < 1.0).then_some(r_out_perp + r_out_parallel)
}

