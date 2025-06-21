use image::{RgbImage, Rgb};
use rand::{thread_rng, Rng};
use rand::distributions::{Alphanumeric, Uniform};
use std::f32::consts::PI;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;

#[derive(Copy, Clone)]
struct Vec3(f32, f32, f32);
impl Vec3 {
    // basic vector ops (add, sub, dot, cross, normalize, length, etc.)
    fn add(self, v: Vec3) -> Vec3 { Vec3(self.0 + v.0, self.1 + v.1, self.2 + v.2) }
    fn sub(self, v: Vec3) -> Vec3 { Vec3(self.0 - v.0, self.1 - v.1, self.2 - v.2) }
    fn scale(self, f: f32) -> Vec3 { Vec3(self.0 * f, self.1 * f, self.2 * f) }
    fn dot(self, v: Vec3) -> f32 { self.0 * v.0 + self.1 * v.1 + self.2 * v.2 }
    fn cross(self, v: Vec3) -> Vec3 {
        Vec3(
            self.1 * v.2 - self.2 * v.1,
            self.2 * v.0 - self.0 * v.2,
            self.0 * v.1 - self.1 * v.0,
        )
    }
    fn norm(self) -> f32 { self.dot(self).sqrt() }
    fn normalize(self) -> Vec3 { let n = self.norm(); self.scale(1.0 / n) }
    fn neg(self) -> Vec3 { Vec3(-self.0, -self.1, -self.2) }
}

fn reflect(v: Vec3, n: Vec3) -> Vec3 {
    v.sub(n.scale(2.0 * v.dot(n)))
}

// GGX microfacet functions
fn fresnel_schlick(cos_theta: f32, f0: Vec3) -> Vec3 {
    f0.add(Vec3(1.0,1.0,1.0).sub(f0).scale((1.0 - cos_theta).powf(5.0)))
}

// D term
fn ggx_d(n_dot_h: f32, alpha: f32) -> f32 {
    let a2 = alpha*alpha;
    a2 / (PI * ((n_dot_h*n_dot_h*(a2-1.0)+1.0).powf(2.0)))
}

// geometry (Smith correlated)
fn ggx_g(n_dot_v: f32, n_dot_l: f32, alpha: f32) -> f32 {
    let g1 = 2.0 / (1.0 + (1.0 + alpha * alpha * (1.0 - n_dot_v*n_dot_v) / (n_dot_v*n_dot_v)).sqrt());
    let g2 = 2.0 / (1.0 + (1.0 + alpha * alpha * (1.0 - n_dot_l*n_dot_l) / (n_dot_l*n_dot_l)).sqrt());
    g1 * g2
}

// Uniform sample on disk for DOF
fn sample_disk(r: f32) -> (f32, f32) {
    let r2 = r * r;
    let phi = 2.0 * PI * thread_rng().r#gen::<f32>();
    (phi.cos() * r2.sqrt(), phi.sin() * r2.sqrt())
}

#[derive(Clone, Copy)]
struct Material {
    color: Vec3,          // diffuse
    metallic: f32,
    roughness: f32,
}

enum Object {
    Sphere { center: Vec3, radius: f32, material: Material },
    Plane  { point: Vec3, normal: Vec3, material: Material },
}

fn intersect(obj: &Object, ro: Vec3, rd: Vec3) -> Option<(f32, Vec3, Material)> {
    match *obj {
        Object::Sphere { center, radius, material } => {
            let oc = ro.sub(center);
            let a = rd.dot(rd);
            let b = 2.0 * oc.dot(rd);
            let c = oc.dot(oc) - radius * radius;
            let disc = b * b - 4.0 * a * c;
            if disc < 0.0 { return None; }
            let t = (-b - disc.sqrt()) / (2.0 * a);
            if t > 0.0 {
                let hit = ro.add(rd.scale(t));
                let n = hit.sub(center).normalize();
                return Some((t, n, material));
            }
            None
        }
        Object::Plane { point, normal, material } => {
            let denom = normal.dot(rd);
            if denom.abs() < 1e-6 { return None; }
            let t = point.sub(ro).dot(normal) / denom;
            if t > 0.0 {
                return Some((t, normal, material));
            }
            None
        }
    }
}

// Area light sampling
struct Light { pos: Vec3, u: Vec3, v: Vec3, intensity: Vec3 }
fn lighting(hit: Vec3, n: Vec3, v: Vec3, mat: Material, objects: &[Object], light: &Light) -> Vec3 {
    let mut diff = Vec3(0.0,0.0,0.0);
    let mut spec = Vec3(0.0,0.0,0.0);
    let samples = 8;

    for _ in 0..samples {
        let u_off = thread_rng().r#gen::<f32>();
        let v_off = thread_rng().r#gen::<f32>();
        let lp = light.pos.add(light.u.scale(u_off - 0.5)).add(light.v.scale(v_off - 0.5));
        let ld = lp.sub(hit);
        let dist2 = ld.dot(ld);
        let l = ld.normalize();

        // shadow test
        let shadow_ro = hit.add(n.scale(0.001));
        let mut blocked = false;
        for o in objects {
            if let Some((t, _, _)) = intersect(o, shadow_ro, l) {
                if t*t < dist2 { blocked = true; break; }
            }
        }
        if blocked { continue; }

        let n_dot_l = n.dot(l).max(0.0);
        diff = diff.add(mat.color.scale(n_dot_l));

        // microfacet
        let h = l.add(v).normalize();
        let n_dot_v = n.dot(v).max(1e-4);
        let n_dot_l2 = n.dot(l).max(1e-4);
        let n_dot_h = n.dot(h).max(0.0);
        let v_dot_h = v.dot(h).max(0.0);

        let f0 = Vec3(0.04,0.04,0.04).add(mat.color.scale(mat.metallic));
        let f = fresnel_schlick(v_dot_h, f0);
        let d = ggx_d(n_dot_h, mat.roughness);
        let g = ggx_g(n_dot_v, n_dot_l2, mat.roughness);
        let spec_c = f.scale(d * g / (4.0 * n_dot_v * n_dot_l2));

        spec = spec.add(spec_c.scale(n_dot_l));
    }

    diff = diff.scale(light.intensity.0 / samples as f32);
    spec = spec.scale(light.intensity.0 / samples as f32);
    diff.add(spec)
}

fn tone(c: Vec3) -> Vec3 {
    // simple Reinhard
    let x = c.scale(1.0 / (1.0 + c.0));
    Vec3(x.0, x.1, x.2)
}

fn render(ro: Vec3, rd: Vec3, objects: &[Object], light: &Light) -> Vec3 {
    if let Some((t,n,mat)) = objects.iter().filter_map(|o| intersect(o, ro, rd)).min_by(|a,b| a.0.partial_cmp(&b.0).unwrap()) {
        let hit = ro.add(rd.scale(t));
        let v = rd.neg().normalize();
        let base = lighting(hit, n, v, mat, objects, light).scale(1.0 / PI);
        base
    } else {
        let t = 0.5 * (rd.1 + 1.0);
        Vec3(1.0,1.0,1.0).scale(1.0 - t).add(Vec3(0.5,0.7,1.0).scale(t))
    }
}

fn render_image_name(width: u32, height: u32, samples: u32, aperture: f32, focus: f32) -> String {
    let suffix: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(6)
        .map(char::from)
        .collect();
    format!("render_{}x{}_s{}_ap{:.2}_f{:.1}_{}.jpg", width, height, samples, aperture, focus, suffix)
}

fn trace(ro: Vec3, rd: Vec3, objects: &[Object], light: &Light, depth: u32) -> Vec3 {
    if depth >= 5 { return Vec3(0.0,0.0,0.0); }
    if let Some((t, n, mat)) = intersect_closest(ro, rd, objects) {
        let hit = ro.add(rd.scale(t));
        // If glass-like
        if mat.roughness < 0.05 && mat.metallic < 0.5 {
            let reflect_dir = reflect(rd, n).normalize();
            let refract_dir = refract(rd, n, 1.0 / 1.5).map(|v| v.normalize());
            let fres = fresnel_schlick(rd.neg().dot(n).max(0.0), Vec3(1.0,1.0,1.0));
            let refl = trace(hit.add(n.scale(0.001)), reflect_dir, objects, light, depth+1);
            let refr = if let Some(rd2) = refract_dir {
                trace(hit.sub(n.scale(0.001)), rd2, objects, light, depth+1)
            } else { Vec3(0.0,0.0,0.0) };
            return refl.scale(fres.0) // approximate scalar blending
                .add(refr.scale(1.0 - fres.0));
        }

        // Diffuse/Glossy surfaces: sample direct lighting + next bounce
        let direct = lighting(hit, n, rd.neg(), mat, objects, light);
        // Next bounce direction: cosine-weighted hemisphere
        let w = n;
        let u = if w.0.abs() > 0.1 {
            w.cross(Vec3(0.0,1.0,0.0)).normalize()
        } else {
            w.cross(Vec3(1.0,0.0,0.0)).normalize()
        };
        let v = w.cross(u);
        let r1 = thread_rng().r#gen::<f32>();
        let r2 = thread_rng().r#gen::<f32>();
        let phi = 2.0 * PI * r1;
        let cos_t = (1.0 - r2).sqrt();
        let sin_t = r2.sqrt();
        let hemi_dir = u.scale(phi.cos() * sin_t)
            .add(v.scale(phi.sin() * sin_t))
            .add(w.scale(cos_t))
            .normalize();
        let indirect = trace(hit.add(n.scale(0.001)), hemi_dir, objects, light, depth+1);
        let albedo = mat.color;
        return direct.add(indirect.scale(albedo.0));
    }
    // Environment
    let t = 0.5 * (rd.1 + 1.0);
    Vec3(1.0,1.0,1.0).scale(1.0 - t).add(Vec3(0.5,0.7,1.0).scale(t))
}
fn intersect_closest(ro: Vec3, rd: Vec3, objects: &[Object]) -> Option<(f32, Vec3, Material)> {
    objects.iter()
        .filter_map(|obj| intersect(obj, ro, rd))
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
}

fn refract(dir: Vec3, normal: Vec3, eta: f32) -> Option<Vec3> {
    let cos_i = -dir.dot(normal).max(-1.0).min(1.0);
    let mut n = normal;
    let mut eta_i = 1.0;
    let mut eta_t = eta;

    // Inside the object?
    if cos_i < 0.0 {
        n = n.neg();
        eta_i = eta;
        eta_t = 1.0;
    }

    let eta_ratio = eta_i / eta_t;
    let k = 1.0 - eta_ratio * eta_ratio * (1.0 - cos_i * cos_i);
    if k < 0.0 {
        None // total internal reflection
    } else {
        Some(dir.scale(eta_ratio).add(n.scale(eta_ratio * cos_i - k.sqrt())))
    }
}

fn main() {
    let width=800; let height=600; 
    let samples=1024;
    let focus= 0.5;
    let aperture = 0.02;  // small value, less blur
    let fov=60f32.to_radians();
    let aspect=width as f32 / height as f32;
    let scale=(fov*0.5).tan();

    let cam = Vec3(0.0,0.0,0.0);
    let up = Vec3(0.0,1.0,0.0);
    let forward = Vec3(0.0,0.0,1.0);
    let right = forward.cross(up).normalize();
    let real_up = right.cross(forward).normalize();

    let light = Light {
        pos: Vec3(0.0, 2.95, 4.0),         // near ceiling center
        u: Vec3(1.0, 0.0, 0.0),            // horizontal extent
        v: Vec3(0.0, 0.0, 1.0),            // depth extent
        intensity: Vec3(25.0, 25.0, 25.0), // bright white
    };

    let red = Material { color: Vec3(1.0, 0.0, 0.0), metallic: 0.0, roughness: 1.0 };
    let green = Material { color: Vec3(0.0, 1.0, 0.0), metallic: 0.0, roughness: 1.0 };
    let white = Material { color: Vec3(1.0, 1.0, 1.0), metallic: 0.0, roughness: 1.0 };
    let mirror = Material { color: Vec3(1.0, 1.0, 1.0), metallic: 1.0, roughness: 0.01 };
    let glass = Material { color: Vec3(1.0, 1.0, 1.0), metallic: 0.0, roughness: 0.01 };

    let objects = Arc::new(vec![
        // Room (Cornell box without ceiling center)
        Object::Plane { point: Vec3(0.0, -1.0, 0.0), normal: Vec3(0.0, 1.0, 0.0), material: white }, // floor
        Object::Plane { point: Vec3(0.0, 3.0, 0.0), normal: Vec3(0.0, -1.0, 0.0), material: white }, // ceiling around hole (you can add edges if needed)
        Object::Plane { point: Vec3(0.0, 0.0, 8.0), normal: Vec3(0.0, 0.0, -1.0), material: white }, // back wall
        Object::Plane { point: Vec3(-3.0, 0.0, 0.0), normal: Vec3(1.0, 0.0, 0.0), material: red },   // left
        Object::Plane { point: Vec3(3.0, 0.0, 0.0), normal: Vec3(-1.0, 0.0, 0.0), material: green }, // right

        // Reflective sphere
        Object::Sphere {
            center: Vec3(-0.8, -0.2, 4.0),
            radius: 0.9,
            material: mirror,
        },

        // Glass sphere
        Object::Sphere {
            center: Vec3(1.0, -0.3, 4.2),
            radius: 0.7,
            material: glass,
        },
    ]);



    /*// Compute bounding box
    let mut min = Vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = Vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);

    for obj in objects.iter() {
        if let Object::Sphere { center, radius, .. } = obj {
            let r = *radius;
            min = Vec3(
                min.0.min(center.0 - r),
                min.1.min(center.1 - r),
                min.2.min(center.2 - r),
            );
            max = Vec3(
                max.0.max(center.0 + r),
                max.1.max(center.1 + r),
                max.2.max(center.2 + r),
            );
        }
    }


    // Focus point = center of bbox
    let center = Vec3(
        (min.0 + max.0) * 0.5,
        (min.1 + max.1) * 0.5,
        (min.2 + max.2) * 0.5,
    );
    let focus = center.sub(cam).norm();
    println!("Focus to bounding box center: {:.2}", focus);*/
    
    /*// Autofocus (adjust for surface offset)
    let autofocus_dir = Vec3(0.0, 0.0, 1.0);
    let focus = if let Some((t, n, _)) = objects.iter()
        .filter_map(|o| intersect(o, cam, autofocus_dir))
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
    {
        let hit_point = cam.add(autofocus_dir.scale(t));
        let padded = hit_point.add(n.scale(0.1)); // push focus behind surface
        (padded.sub(cam)).norm()
    } else {
        5.0
    };
    println!("Adjusted focus: {:.2}", focus);*/

    /*let f_stop = 4.0; // adjust this like a real lens
    let aperture = focus / (2.0 * f_stop);
    println!("Adaptive aperture (f/{:.1}): {:.4}", f_stop, aperture);*/


    let progress = Arc::new(ProgressBar::new(height as u64));
    progress.set_style(ProgressStyle::default_bar()
        .template("{bar:40.cyan/blue} {pos}/{len} rows")
        .unwrap()
        .progress_chars("##-"));
    
    
    let mut img = RgbImage::new(width, height);
    
    let pixels: Vec<((u32, u32), [u8; 3])> = (0..height).into_par_iter().flat_map(|y| {
        progress.inc(1);
        let mut rng = thread_rng();
        let mut row = Vec::with_capacity(width as usize);
        for x in 0..width {
            let mut col = Vec3(0.0, 0.0, 0.0);
            let sqrt_samples = (samples as f32).sqrt() as u32;

            for i in 0..sqrt_samples {
                for j in 0..sqrt_samples {
                    let jitter_x = (i as f32 + rng.r#gen::<f32>()) / sqrt_samples as f32;
                    let jitter_y = (j as f32 + rng.r#gen::<f32>()) / sqrt_samples as f32;

                    let u = ((x as f32 + jitter_x) / width as f32 - 0.5) * 2.0 * aspect * scale;
                    let v = -((y as f32 + jitter_y) / height as f32 - 0.5) * 2.0 * scale;

                    let rd0 = Vec3(u, v, 1.0).normalize();
                    let (dx, dy) = sample_disk(rng.r#gen::<f32>() * aperture);
                    let origin = cam.add(right.scale(dx)).add(real_up.scale(dy)).add(rd0.scale(focus));
                    let rd = origin.sub(cam).normalize();

                    col = col.add(trace(origin, rd, &objects, &light, 0));

                }
            }


            col = tone(col.scale(1.0 / samples as f32));
            let rgb = [
                (col.0 * 255.0).min(255.0) as u8,
                (col.1 * 255.0).min(255.0) as u8,
                (col.2 * 255.0).min(255.0) as u8,
            ];
            row.push(((x, y), rgb));
        }
        row
    }).collect();

    // Write pixels serially
    for ((x, y), rgb) in pixels {
        img.put_pixel(x, y, Rgb(rgb));
    }

    let filename = render_image_name(width, height, samples, aperture, focus);


    progress.finish_with_message("Rendering complete");
    
    img.save(&filename).unwrap();
    println!("Saved to {}", filename);
}
