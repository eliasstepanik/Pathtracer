// C:\Users\Elias Stepanik\RustroverProjects\Pathtracer\src\gpu_pathtrace.wgsl

const PI: f32 = 3.1415926535;
const SHADOW_SAMPLES: u32 = 4u;

struct Camera { pos: vec4<f32>, forward: vec4<f32>, up: vec4<f32>, right: vec4<f32>, width: u32, height: u32, fov: f32, sphere_count: u32, plane_count: u32, triangle_count: u32, aperture: f32, focus_dist: f32 };
struct RenderParams { samples_per_pixel: u32, max_bounces: u32, seed1: u32, seed2: u32 };
struct Light { pos: vec4<f32>, intensity: vec4<f32>, u: vec4<f32>, v: vec4<f32> };
struct Sphere { center: vec4<f32>, color: vec4<f32>, radius: f32, metallic: f32, roughness: f32, ior: f32 };
struct Plane { point: vec4<f32>, normal: vec4<f32>, u: vec4<f32>, v: vec4<f32>, color: vec4<f32>, metallic: f32, roughness: f32, ior: f32, _pad: f32 };
struct Triangle { v0: vec4<f32>, v1: vec4<f32>, v2: vec4<f32>, normal: vec4<f32>, color: vec4<f32>, metallic: f32, roughness: f32, ior: f32, _pad: f32 };

// --- START: BUG FIX ---
// The output buffer must match the data type the CPU expects for accumulation.
@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var<uniform> params: RenderParams;
@group(0) @binding(2) var<uniform> light: Light;
@group(0) @binding(3) var<storage, read> spheres: array<Sphere>;
@group(0) @binding(4) var<storage, read> planes: array<Plane>;
@group(0) @binding(5) var<storage, read> triangles: array<Triangle>;
@group(0) @binding(6) var<storage, read_write> output: array<vec4<f32>>;
// --- END: BUG FIX ---

struct Ray { origin: vec3<f32>, dir: vec3<f32> };
struct Material { color: vec3<f32>, metallic: f32, roughness: f32, ior: f32 };
struct HitInfo { t: f32, pos: vec3<f32>, normal: vec3<f32>, mat: Material };

var<private> rand_seed: vec2<u32>;
fn pcg_hash(input: u32) -> u32 { var state = input * 747796405u + 2891336453u; let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u; return (word >> 22u) ^ word; }
fn init_rand(invocation_id: vec2<u32>, seed: vec2<u32>) { rand_seed = vec2<u32>(pcg_hash(invocation_id.x) ^ seed.x, pcg_hash(invocation_id.y) ^ seed.y); }
fn rand() -> f32 { rand_seed.x = pcg_hash(rand_seed.x); rand_seed.y = pcg_hash(rand_seed.y); return f32(pcg_hash(rand_seed.x ^ rand_seed.y)) / 4294967295.0; }
fn rand_in_unit_disk() -> vec2<f32> { for (var i = 0; i < 10; i = i + 1) { let p = vec2(rand(), rand()) * 2.0 - 1.0; if (dot(p, p) < 1.0) { return p; } } return vec2(0.0, 0.0); }

fn reflect(v: vec3<f32>, n: vec3<f32>) -> vec3<f32> { return v - n * 2.0 * dot(v, n); }
fn refract(v: vec3<f32>, n: vec3<f32>, eta_ratio: f32) -> vec3<f32> { let cos_theta = min(dot(-v, n), 1.0); let r_out_perp = (v + n * cos_theta) * eta_ratio; let discriminant = 1.0 - dot(r_out_perp, r_out_perp); if (discriminant < 0.0) { return vec3(0.0); } let r_out_parallel = n * -sqrt(discriminant); return r_out_perp + r_out_parallel; }
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> { return f0 + (vec3(1.0) - f0) * pow(1.0 - cos_theta, 5.0); }
fn d_term(nh: f32, a: f32) -> f32 { let a2 = a * a; return a2 / (PI * pow(nh * nh * (a2 - 1.0) + 1.0, 2.0)); }
fn g_term(nv: f32, nl: f32, a: f32) -> f32 { let k = a * a / 2.0; let g1 = nv / (nv * (1.0 - k) + k); let g2 = nl / (nl * (1.0 - k) + k); return g1 * g2; }
fn any_orthonormal(n: vec3<f32>) -> vec3<f32> { if (abs(n.z) < 0.9999999) { return vec3(n.y, -n.x, 0.0); } return vec3(0.0, -n.z, n.y); }
fn sample_ggx_h(n: vec3<f32>, roughness: f32) -> vec3<f32> { let a = roughness * roughness; let a2 = a * a; let r1 = rand(); let r2 = rand(); let phi = 2.0 * PI * r1; let cos_theta = sqrt((1.0 - r2) / (1.0 + (a2 - 1.0) * r2)); let sin_theta = sqrt(max(0.0, 1.0 - cos_theta * cos_theta)); let h_tangent = vec3(cos(phi) * sin_theta, sin(phi) * sin_theta, cos_theta); let w = n; let u = normalize(any_orthonormal(n)); let v = cross(w, u); return u * h_tangent.x + v * h_tangent.y + w * h_tangent.z; }
fn sample_hemisphere(n: vec3<f32>) -> vec3<f32> { let w = n; let u = normalize(any_orthonormal(w)); let v = cross(w, u); let phi = 2.0 * PI * rand(); let r2 = rand(); let r2s = sqrt(r2); return normalize(u * cos(phi) * r2s + v * sin(phi) * r2s + w * sqrt(1.0 - r2)); }

fn intersect_sphere(ray: Ray, s: Sphere, t_min: f32, t_max: f32) -> f32 { let oc = ray.origin - s.center.xyz; let a = dot(ray.dir, ray.dir); let b = 2.0 * dot(oc, ray.dir); let c = dot(oc, oc) - s.radius * s.radius; let disc = b * b - 4.0 * a * c; if (disc < 0.0) { return t_max; } let t = (-b - sqrt(disc)) / (2.0 * a); if (t > t_min && t < t_max) { return t; } let t2 = (-b + sqrt(disc)) / (2.0 * a); if (t2 > t_min && t2 < t_max) { return t2; } return t_max; }
fn intersect_plane(ray: Ray, p: Plane, t_min: f32, t_max: f32) -> f32 { let denom = dot(p.normal.xyz, ray.dir); if (abs(denom) < 1e-6) { return t_max; } let t = dot(p.point.xyz - ray.origin, p.normal.xyz) / denom; if (t <= t_min || t >= t_max) { return t_max; } let hit_pos = ray.origin + ray.dir * t; let d = hit_pos - p.point.xyz; if (abs(dot(d, p.u.xyz)) > dot(p.u.xyz, p.u.xyz)) { return t_max; } if (abs(dot(d, p.v.xyz)) > dot(p.v.xyz, p.v.xyz)) { return t_max; } return t; }
fn intersect_triangle(ray: Ray, tri: Triangle, t_min: f32, t_max: f32) -> f32 { let e1 = tri.v1.xyz - tri.v0.xyz; let e2 = tri.v2.xyz - tri.v0.xyz; let p = cross(ray.dir, e2); let det = dot(e1, p); if (abs(det) < 1e-8) { return t_max; } let inv_det = 1.0 / det; let tvec = ray.origin - tri.v0.xyz; let u = dot(tvec, p) * inv_det; if (u < 0.0 || u > 1.0) { return t_max; } let q = cross(tvec, e1); let v = dot(ray.dir, q) * inv_det; if (v < 0.0 || u + v > 1.0) { return t_max; } let t = dot(e2, q) * inv_det; if (t > t_min && t < t_max) { return t; } return t_max; }
fn intersect_scene(ray: Ray) -> HitInfo { var hit: HitInfo; hit.t = 1e9; var closest = 1e9; for (var i = 0u; i < camera.sphere_count; i = i + 1u) { let s = spheres[i]; let t = intersect_sphere(ray, s, 0.001, closest); if (t < closest) { closest = t; hit.t = t; hit.pos = ray.origin + ray.dir * t; hit.normal = normalize(hit.pos - s.center.xyz); hit.mat.color = s.color.xyz; hit.mat.metallic = s.metallic; hit.mat.roughness = s.roughness; hit.mat.ior = s.ior; } } for (var i = 0u; i < camera.plane_count; i = i + 1u) { let p = planes[i]; let t = intersect_plane(ray, p, 0.001, closest); if (t < closest) { closest = t; hit.t = t; hit.pos = ray.origin + ray.dir * t; hit.normal = p.normal.xyz; hit.mat.color = p.color.xyz; hit.mat.metallic = p.metallic; hit.mat.roughness = p.roughness; hit.mat.ior = p.ior; } } for (var i = 0u; i < camera.triangle_count; i = i + 1u) { let tri = triangles[i]; let t = intersect_triangle(ray, tri, 0.001, closest); if (t < closest) { closest = t; hit.t = t; hit.pos = ray.origin + ray.dir * t; hit.normal = tri.normal.xyz; hit.mat.color = tri.color.xyz; hit.mat.metallic = tri.metallic; hit.mat.roughness = tri.roughness; hit.mat.ior = tri.ior; } } return hit; }

fn direct_light_sample(hit_pos: vec3<f32>, hit_normal: vec3<f32>, mat: Material, v: vec3<f32>) -> vec3<f32> { var total_direct_light = vec3(0.0); for (var i = 0u; i < SHADOW_SAMPLES; i = i + 1u) { let lp = light.pos.xyz + light.u.xyz * (rand() - 0.5) + light.v.xyz * (rand() - 0.5); let lvec = lp - hit_pos; let dist = length(lvec); let l = lvec / dist; var shadow_ray: Ray; shadow_ray.origin = hit_pos + hit_normal * 0.0001; shadow_ray.dir = l; let shadow_hit = intersect_scene(shadow_ray); if (shadow_hit.t >= dist) { let n_dot_l = max(0.0, dot(hit_normal, l)); if (n_dot_l > 0.0) { let light_area = length(cross(light.u.xyz, light.v.xyz)); let light_normal = normalize(cross(light.u.xyz, light.v.xyz)); let cos_theta_light = max(0.0, dot(-l, light_normal)); let falloff = cos_theta_light / (dist * dist + 1.0); let h = normalize(v + l); let n_dot_v = max(1e-4, dot(hit_normal, v)); let n_dot_h = max(0.0, dot(hit_normal, h)); let v_dot_h = max(0.0, dot(v, h)); let f0 = mix(vec3(0.04), mat.color, mat.metallic); let f = fresnel_schlick(v_dot_h, f0); let d = d_term(n_dot_h, mat.roughness); let g = g_term(n_dot_v, n_dot_l, mat.roughness); let spec_numerator = f * d * g; let spec_denominator = 4.0 * n_dot_v * n_dot_l + 1e-6; let specular_brdf = spec_numerator / spec_denominator; let diffuse_color = mat.color * (1.0 - mat.metallic); let k_d = vec3(1.0) - f; let diffuse_brdf = diffuse_color * k_d / PI; let radiance = (diffuse_brdf + specular_brdf) * n_dot_l; total_direct_light += radiance * light.intensity.xyz * light_area * falloff; } } } return total_direct_light / f32(SHADOW_SAMPLES); }
fn aces_film(c: vec3<f32>) -> vec3<f32> { let a = 2.51; let b = 0.03; let c2 = 2.43; let d = 0.59; let e = 0.14; let x = (c * (a * c + b)) / (c * (c2 * c + d) + e); return clamp(x, vec3(0.0), vec3(1.0)); }

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= camera.width || gid.y >= camera.height) { return; }
    init_rand(gid.xy, vec2(params.seed1, params.seed2));
    var final_color = vec3(0.0);
    for (var s = 0u; s < params.samples_per_pixel; s = s + 1u) {
        let aspect = f32(camera.width) / f32(camera.height);
        let scale = tan(radians(camera.fov) * 0.5);
        let right = camera.right.xyz;
        let up = camera.up.xyz;
        let u_offset = ( (f32(gid.x) + rand()) / f32(camera.width) - 0.5) * 2.0 * aspect * scale;
        let v_offset = -( (f32(gid.y) + rand()) / f32(camera.height) - 0.5) * 2.0 * scale;
        let rd0 = normalize(right * u_offset + up * v_offset + camera.forward.xyz);
        let lens_rand = rand_in_unit_disk() * camera.aperture;
        let origin_offset = right * lens_rand.x + up * lens_rand.y;
        let focal_pt = camera.pos.xyz + rd0 * camera.focus_dist;
        var current_ray: Ray;
        current_ray.origin = camera.pos.xyz + origin_offset;
        current_ray.dir = normalize(focal_pt - current_ray.origin);
        var throughput = vec3(1.0);

        for (var i = 0u; i < params.max_bounces; i = i + 1u) {
            let hit = intersect_scene(current_ray);
            if (hit.t >= 1e9) { break; }

            let view_dir = -current_ray.dir;

            if (hit.mat.ior > 1.0 && hit.mat.metallic < 0.1) {
                var hit_normal = hit.normal;
                let cosi = dot(view_dir, hit_normal);
                var etai = 1.0;
                var etat = hit.mat.ior;
                if (cosi < 0.0) {
                    hit_normal = -hit_normal;
                    etai = hit.mat.ior;
                    etat = 1.0;
                }

                let eta_ratio = etai / etat;
                let r0 = pow((etai - etat) / (etai + etat), 2.0);
                let reflectance = r0 + (1.0 - r0) * pow(1.0 - abs(cosi), 5.0);
                if (rand() < reflectance) {
                    current_ray.dir = reflect(-view_dir, hit_normal);
                } else {
                    let refract_dir = refract(-view_dir, hit_normal, eta_ratio);
                    if(dot(refract_dir, refract_dir) > 0.0) {
                        current_ray.dir = refract_dir;
                    } else {
                        current_ray.dir = reflect(-view_dir, hit_normal);
                    }
                }
                throughput *= hit.mat.color;
            } else {
                let hit_normal = select(hit.normal, -hit.normal, dot(hit.normal, view_dir) < 0.0);
                final_color += direct_light_sample(hit.pos, hit_normal, hit.mat, view_dir) * throughput;

                var next_dir: vec3<f32>;
                let diffuse_chance = 1.0 - hit.mat.metallic;
                if (rand() < diffuse_chance) {
                    next_dir = sample_hemisphere(hit_normal);
                    throughput *= hit.mat.color;
                } else {
                    let h = sample_ggx_h(hit_normal, hit.mat.roughness);
                    next_dir = reflect(-view_dir, h);
                    let f0 = mix(vec3(0.04), hit.mat.color, hit.mat.metallic);
                    throughput *= fresnel_schlick(max(0.0, dot(h, view_dir)), f0);
                }
                current_ray.dir = next_dir;
            }

            if (i > 1u) {
                let p = max(throughput.x, max(throughput.y, throughput.z));
                if (rand() > p) { break; }
                throughput /= p;
            }
            current_ray.origin = hit.pos + current_ray.dir * 0.0001;
        }
    }

    // --- START: BUG FIX ---
    // The shader now returns the SUM of colors for its chunk of samples.
    // The CPU will handle averaging, tonemapping, and gamma correction.
    let index = gid.y * camera.width + gid.x;
    output[index] = vec4(final_color, 1.0);
    // --- END: BUG FIX ---
}