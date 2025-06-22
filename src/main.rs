mod algebra;
mod material;
mod ggx;
mod object;
mod light;
mod scene;
mod tonemap;
mod renderer;
mod plane;
mod sphere;
mod gpu_types;

use crate::{
    renderer::render_image_name,
    algebra::{sample_disk, Vec3},
};
use image::{Rgb, RgbImage};
use indicatif::{ProgressBar, ProgressStyle};
use rand::thread_rng;
use rayon::prelude::*;
use std::sync::Arc;
use crate::scene::load;

const MAX_DEPTH: u32 = 12;
const MAX_GLASS_BOUNCES: u32 = 8;

fn main() {
    // ── parse JSON ────────────────────────────────────────────────────────
    let scene = load("scene.json");

    let width     = scene.render.width;
    let height    = scene.render.height;
    let samples   = scene.render.samples;
    let aperture  = scene.camera.aperture;
    let fov_rad   = scene.camera.fov.to_radians();

    // camera basis
    let aspect = width as f32 / height as f32;
    let scale  = (fov_rad * 0.5).tan();
    let pos    = scene.camera.pos;
    let look_at= scene.camera.look_at;

    let up_v   = scene.camera.up;

    let forward = look_at.sub(pos).normalize();
    // Right = Up  × Forward   (right-handed basis)
    let right   = up_v.cross(forward).normalize();
    let real_up = forward.cross(right).normalize();

    // autofocus
    // --- MODIFIED: Pass the full camera basis to autofocus ---
    let focus = renderer::autofocus(
        pos, right, real_up, forward,
        aspect, scale, width, height, &scene.objects);


    // ── dump debug info ────────────────────────────────────────────────────
    println!("=== CAMERA INFO ===");
    println!(" position : {:?}", pos);
    println!(" look_at  : {:?}", look_at);
    println!(" up       : {:?}", up_v);
    println!(" fov (°)  : {:.2}", scene.camera.fov);
    println!(" fov (rad): {:.4}", fov_rad);
    println!(" aspect   : {:.4}", aspect);
    println!(" scale    : {:.4}", scale);
    println!(" forward  : {:?}", forward);
    println!(" right    : {:?}", right);
    println!(" real_up  : {:?}", real_up);
    println!(" aperture : {:.4}", aperture);
    println!(" autofocus: {:.4}", focus);

    println!("\n=== OBJECTS ({}) ===", scene.objects.len());
    for (i, obj) in scene.objects.iter().enumerate() {
        match obj {
            crate::object::Object::Sphere(s) => {
                println!(" [{}] Sphere '{}' {{ center: {:?}, radius: {:.4}, mat_color: {:?} }}",
                         i, s.name, s.center, s.radius, s.material.color);
            }
            crate::object::Object::Plane(p) => {
                println!(" [{}] Plane '{}' {{ point: {:?}, normal: {:?}, mat_color: {:?} }}",
                         i, p.name, p.point, p.normal, p.material.color);
            }
        }
    }

    println!("\n=== LIGHTS ({}) ===", scene.lights.len());
    for (i, l) in scene.lights.iter().enumerate() {
        println!(" [{}] Light {{ pos: {:?}, u: {:?}, v: {:?}, intensity: {:?} }}",
                 i, l.pos, l.u, l.v, l.intensity);
    }




    // ── multithreaded render loop ─────────────────────────────────────────
    let bar = ProgressBar::new(height as u64);
    bar.set_style(ProgressStyle::default_bar()
        .template("{bar:40.cyan/blue} {pos}/{len} rows").unwrap());

    let objects = Arc::new(scene.objects);
    let lights  = Arc::new(scene.lights);




    let mut img = RgbImage::new(width, height);
    let rows: Vec<_> = (0..height).into_par_iter().flat_map(|y| {
        bar.inc(1);
        let mut rng = thread_rng();
        let mut row = Vec::with_capacity(width as usize);

        for x in 0..width {
            // --- MODIFIED: Pass the `forward` vector to pixel_color ---
            let col = renderer::pixel_color(
                x, y, width, height, samples, aspect, scale,
                pos, right, real_up, forward, focus, aperture,
                &objects, &lights, &mut rng);
            row.push(((x, y), col));
        }
        row
    }).collect();

    bar.finish_with_message("Rendering complete");

    for ((x, y), rgb) in rows { img.put_pixel(x, y, Rgb(rgb)); }
    let name = render_image_name(width, height, samples, aperture, focus);
    img.save(&name).unwrap();
    println!("Saved → {name}");
}