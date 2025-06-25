// In C:\Users\Elias Stepanik\RustroverProjects\Pathtracer\src\gpu_pathtrace.wgsl

// --- Constants ---
const PI: f32 = 3.1415926535;
const SHADOW_SAMPLES: u32 = 4u;

// --- Data Structs from Host ---

struct Camera {
    pos: vec3<f32>,
    _pad0: f32,
    look_at: vec3<f32>,
    _pad1: f32,
    up: vec3<f32>,
    _pad2: f32,
    forward: vec3<f32>,
    _pad3: f32,
    width: u32,
    height: u32,
    fov: f32,
    sphere_count: u32,
    plane_count: u32,
    aperture: f32,
    focus_dist: f32,
    _pad4: u32,
};

struct RenderParams {
    samples_per_pixel: u32,
    max_bounces: u32,
    seed1: u32,
    seed2: u32,
};

struct Light {
    pos: vec3<f32>,
    _pad0: f32,
    intensity: vec3<f32>,
    _pad1: f32,
    u: vec3<f32>,
    _pad2: f32,
    v: vec3<f32>,
    _pad3: f32,
};

struct Sphere {
    center: vec3<f32>,
    radius: f32,
    color: vec3<f32>,
    metallic: f32,
    roughness: f32,
    _pad: vec3<f32>,
};

struct Plane {
    point: vec3<f32>,
    _pad0: f32,
    normal: vec3<f32>,
    metallic: f32,
    u: vec3<f32>,
    roughness: f32,
    v: vec3<f32>,
    _pad1: f32,
    color: vec3<f32>,
    _pad2: f32,
};

// --- Bindings ---

@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var<uniform> params: RenderParams;
@group(0) @binding(2) var<uniform> light: Light;
@group(0) @binding(3) var<storage, read> spheres: array<Sphere>;
@group(0) @binding(4) var<storage, read> planes: array<Plane>;
@group(0) @binding(5) var<storage, read_write> output: array<u32>;

// --- Helper Structs ---

struct Ray {
    origin: vec3<f32>,
    dir: vec3<f32>,
};

struct Material {
    color: vec3<f32>,
    metallic: f32,
    roughness: f32,
};

struct HitInfo {
    t: f32,
    pos: vec3<f32>,
    normal: vec3<f32>,
    mat: Material,
};

// --- Random Number Generator ---
var<private> rand_seed: vec2<u32>;

// --- START: MODIFICATION 1 (Robust RNG) ---
fn pcg_hash(input: u32) -> u32 {
    var state = input * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn init_rand(invocation_id: vec2<u32>, seed: vec2<u32>) {
    // Hash the large invocation_id components first to bring them into a good range.
    // Use XOR to combine seeds to prevent overflow.
    rand_seed = vec2<u32>(
        pcg_hash(invocation_id.x) ^ seed.x,
        pcg_hash(invocation_id.y) ^ seed.y
    );
}

fn rand() -> f32 {
    rand_seed.x = pcg_hash(rand_seed.x);
    rand_seed.y = pcg_hash(rand_seed.y);
    // Combine with XOR to prevent overflow.
    return f32(pcg_hash(rand_seed.x ^ rand_seed.y)) / 4294967295.0;
}

fn rand_vec3() -> vec3<f32> {
    return vec3(rand(), rand(), rand());
}
fn rand_in_unit_disk() -> vec2<f32> {
    // Re-structure the loop to a finite `for` loop to guarantee termination
    // for the shader compiler. 10 iterations is more than enough.
    for (var i = 0; i < 10; i = i + 1) {
        let p = vec2(rand(), rand()) * 2.0 - 1.0;
        if (dot(p, p) < 1.0) {
            return p;
        }
    }
    return vec2(0.0, 0.0); // Return a fallback value
}

// --- PBR / GGX Math Functions (Ported from Rust) ---

fn reflect(v: vec3<f32>, n: vec3<f32>) -> vec3<f32> { return v - n * 2.0 * dot(v, n); }

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3(1.0) - f0) * pow(1.0 - cos_theta, 5.0);
}
fn d_term(nh: f32, a: f32) -> f32 {
    let a2 = a * a;
    return a2 / (PI * pow(nh * nh * (a2 - 1.0) + 1.0, 2.0));
}
fn g_term(nv: f32, nl: f32, a: f32) -> f32 {
    let k = a * a / 2.0;
    let g1 = nv / (nv * (1.0 - k) + k);
    let g2 = nl / (nl * (1.0 - k) + k);
    return g1 * g2;
}
fn any_orthonormal(n: vec3<f32>) -> vec3<f32> {
    if (abs(n.z) < 0.9999999) { return vec3(n.y, -n.x, 0.0); }
    return vec3(0.0, -n.z, n.y);
}
fn sample_ggx_h(n: vec3<f32>, roughness: f32) -> vec3<f32> {
    let a = roughness * roughness;
    let a2 = a * a;
    let r1 = rand();
    let r2 = rand();
    let phi = 2.0 * PI * r1;
    let cos_theta = sqrt((1.0 - r2) / (1.0 + (a2 - 1.0) * r2));
    let sin_theta = sqrt(max(0.0, 1.0 - cos_theta * cos_theta));

    let h_tangent = vec3(cos(phi) * sin_theta, sin(phi) * sin_theta, cos_theta);

    let w = n;
    let u = normalize(any_orthonormal(n));
    let v = cross(w, u);
    return u * h_tangent.x + v * h_tangent.y + w * h_tangent.z;
}
fn sample_hemisphere(n: vec3<f32>) -> vec3<f32> {
    let w = n;
    let u = normalize(any_orthonormal(w));
    let v = cross(w, u);
    let phi = 2.0 * PI * rand();
    let r2 = rand();
    let r2s = sqrt(r2);
    return normalize(u * cos(phi) * r2s + v * sin(phi) * r2s + w * sqrt(1.0 - r2));
}

// --- Intersection Functions ---

fn intersect_sphere(ray: Ray, s: Sphere, t_min: f32, t_max: f32) -> f32 {
    let oc = ray.origin - s.center;
    let a = dot(ray.dir, ray.dir);
    let b = 2.0 * dot(oc, ray.dir);
    let c = dot(oc, oc) - s.radius * s.radius;
    let disc = b * b - 4.0 * a * c;
    if (disc < 0.0) { return t_max; }
    let t = (-b - sqrt(disc)) / (2.0 * a);
    if (t > t_min && t < t_max) { return t; }
    let t2 = (-b + sqrt(disc)) / (2.0 * a);
    if (t2 > t_min && t2 < t_max) { return t2; }
    return t_max;
}

fn intersect_plane(ray: Ray, p: Plane, t_min: f32, t_max: f32) -> f32 {
    let denom = dot(p.normal, ray.dir);
    if (abs(denom) < 1e-6) { return t_max; }
    let t = dot(p.point - ray.origin, p.normal) / denom;
    if (t <= t_min || t >= t_max) { return t_max; }

    let hit_pos = ray.origin + ray.dir * t;
    let d = hit_pos - p.point;
    if (abs(dot(d, p.u)) > dot(p.u, p.u)) { return t_max; }
    if (abs(dot(d, p.v)) > dot(p.v, p.v)) { return t_max; }
    return t;
}

fn intersect_scene(ray: Ray) -> HitInfo {
    var hit: HitInfo;
    hit.t = 1e9;
    var closest = 1e9;

    for (var i = 0u; i < camera.sphere_count; i = i + 1u) {
        let s = spheres[i];
        let t = intersect_sphere(ray, s, 0.001, closest);
        if (t < closest) {
            closest = t;
            hit.t = t;
            hit.pos = ray.origin + ray.dir * t;
            hit.normal = normalize(hit.pos - s.center);
            hit.mat.color = s.color;
            hit.mat.metallic = s.metallic;
            hit.mat.roughness = s.roughness;
        }
    }
    for (var i = 0u; i < camera.plane_count; i = i + 1u) {
        let p = planes[i];
        let t = intersect_plane(ray, p, 0.001, closest);
        if (t < closest) {
            closest = t;
            hit.t = t;
            hit.pos = ray.origin + ray.dir * t;
            hit.normal = p.normal;
            hit.mat.color = p.color;
            hit.mat.metallic = p.metallic;
            hit.mat.roughness = p.roughness;
        }
    }
    return hit;
}

// --- Main Shading Logic ---

fn direct_light_sample(hit: HitInfo, v: vec3<f32>) -> vec3<f32> {
    var total_direct_light = vec3(0.0);

    for (var i = 0u; i < SHADOW_SAMPLES; i = i + 1u) {
        let lp = light.pos + light.u * (rand() - 0.5) + light.v * (rand() - 0.5);
        let lvec = lp - hit.pos;
        let dist2 = dot(lvec, lvec);
        let l = normalize(lvec);

        var shadow_ray: Ray;
        shadow_ray.origin = hit.pos;
        shadow_ray.dir = l;
        let shadow_hit = intersect_scene(shadow_ray);

        if (shadow_hit.t * shadow_hit.t > dist2 * 0.999) {
            // ... (the rest of the PBR logic inside the if-statement is the same)
            let n_dot_l = max(0.0, dot(hit.normal, l));
            if (n_dot_l > 0.0) {
                let light_area = length(cross(light.u, light.v));
                let light_normal = normalize(cross(light.u, light.v));
                let cos_theta_light = max(0.0, dot(-l, light_normal));
                let falloff = cos_theta_light / (dist2 + 1e-4);

                let h = normalize(v + l);
                let n_dot_v = max(1e-4, dot(hit.normal, v));
                let n_dot_h = max(0.0, dot(hit.normal, h));
                let v_dot_h = max(0.0, dot(v, h));

                let f0 = mix(vec3(0.04), hit.mat.color, hit.mat.metallic);
                let f = fresnel_schlick(v_dot_h, f0);
                let d = d_term(n_dot_h, hit.mat.roughness);
                let g = g_term(n_dot_v, n_dot_l, hit.mat.roughness);

                let spec_numerator = f * d * g;
                let spec_denominator = 4.0 * n_dot_v * n_dot_l + 1e-6;
                let specular_brdf = spec_numerator / spec_denominator;

                let diffuse_color = hit.mat.color * (1.0 - hit.mat.metallic);
                let k_d = vec3(1.0) - f;
                let diffuse_brdf = diffuse_color * k_d / PI;

                let radiance = (diffuse_brdf + specular_brdf) * n_dot_l;
                total_direct_light += radiance * light.intensity * light_area * falloff;
            }
        }
    }
    return total_direct_light / f32(SHADOW_SAMPLES);
}

fn aces_film(c: vec3<f32>) -> vec3<f32> {
    let a = 2.51; let b = 0.03; let c2 = 2.43; let d = 0.59; let e = 0.14;
    let x = (c * (a * c + b)) / (c * (c2 * c + d) + e);
    return clamp(x, vec3(0.0), vec3(1.0));
}

// --- Main Entry Point ---

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= camera.width || gid.y >= camera.height) { return; }
    init_rand(gid.xy, vec2(params.seed1, params.seed2));

    var final_color = vec3(0.0);

    for (var s = 0u; s < params.samples_per_pixel; s = s + 1u) {
        // --- Generate Camera Ray with Depth of Field ---
        let aspect = f32(camera.width) / f32(camera.height);
        let scale = tan(radians(camera.fov) * 0.5);
        let right = normalize(cross(camera.forward, camera.up));
        let up = cross(right, camera.forward);

        let u_offset = ( (f32(gid.x) + rand()) / f32(camera.width) - 0.5) * 2.0 * aspect * scale;
        let v_offset = -( (f32(gid.y) + rand()) / f32(camera.height) - 0.5) * 2.0 * scale;
        let rd0 = normalize(right * u_offset + up * v_offset + camera.forward);
        let lens_rand = rand_in_unit_disk() * camera.aperture;
        let origin_offset = right * lens_rand.x + up * lens_rand.y;
        let focal_pt = camera.pos + rd0 * camera.focus_dist;

        var current_ray: Ray;
        current_ray.origin = camera.pos + origin_offset;
        current_ray.dir = normalize(focal_pt - current_ray.origin);


        var throughput = vec3(1.0);

        // --- Path Tracing Loop ---
        for (var i = 0u; i < params.max_bounces; i = i + 1u) {
            let hit = intersect_scene(current_ray);
            if (hit.t >= 1e9) {
                // Hit sky (black)
                break;
            }

            let view_dir = -current_ray.dir;
            let hit_normal = select(hit.normal, -hit.normal, dot(hit.normal, view_dir) < 0.0);

            final_color += direct_light_sample(hit, view_dir) * throughput;

            var next_dir: vec3<f32>;

            // --- START: MODIFICATION 2 (Fix Throughput Calculation) ---
            // This new logic correctly and simply updates the throughput,
            // preventing the massive energy loss that caused the dark image.
            let f0 = mix(vec3(0.04), hit.mat.color, hit.mat.metallic);
            let fresnel = fresnel_schlick(max(0.0, dot(hit_normal, view_dir)), f0);

            let diffuse_chance = 1.0 - hit.mat.metallic;
            if (rand() < diffuse_chance) { // Diffuse bounce
                next_dir = sample_hemisphere(hit_normal);
                throughput *= hit.mat.color; // Throughput is multiplied by albedo
            } else { // Specular bounce
                let h = sample_ggx_h(hit_normal, hit.mat.roughness);
                next_dir = reflect(-view_dir, h);
                throughput *= fresnel; // Throughput is multiplied by fresnel reflectance
            }
            // --- END: MODIFICATION 2 ---

            // Russian Roulette to terminate path
            let p = max(throughput.x, max(throughput.y, throughput.z));
            if (rand() > p && i > 1u) {
                break;
            }
            throughput /= p;

            current_ray.origin = hit.pos;
            current_ray.dir = next_dir;
        }
    }

    final_color /= f32(params.samples_per_pixel);
    let tonemapped = aces_film(final_color);
    let gamma_corrected = pow(tonemapped, vec3(1.0/2.2));

    let r = u32(clamp(gamma_corrected.x, 0.0, 1.0) * 255.0);
    let g = u32(clamp(gamma_corrected.y, 0.0, 1.0) * 255.0);
    let b = u32(clamp(gamma_corrected.z, 0.0, 1.0) * 255.0);
    let a = 255u;

    let index = gid.y * camera.width + gid.x;
    output[index] = (a << 24u) | (b << 16u) | (g << 8u) | r;
}