struct Camera {
    pos: vec3<f32>,
    _pad0: f32,
    look_at: vec3<f32>,
    _pad1: f32,
    up: vec3<f32>,
    _pad2: f32,
    width: u32,
    height: u32,
    fov: f32,
    sphere_count: u32,
    plane_count: u32,
    _pad3: vec3<u32>,
};

struct Light {
    pos: vec3<f32>,
    _pad0: f32,
    intensity: vec3<f32>,
    _pad1: f32,
};

struct Sphere {
    center: vec3<f32>,
    radius: f32,
    color: vec3<f32>,
    _pad: f32,
};

struct Plane {
    point: vec3<f32>,
    _pad0: f32,
    normal: vec3<f32>,
    _pad1: f32,
    color: vec3<f32>,
    _pad2: f32,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var<uniform> light: Light;
@group(0) @binding(2) var<storage, read> spheres: array<Sphere>;
@group(0) @binding(3) var<storage, read> planes: array<Plane>;
@group(0) @binding(4) var<storage, read_write> output: array<u32>;

fn intersect_sphere(ro: vec3<f32>, rd: vec3<f32>, s: Sphere) -> f32 {
    let oc = ro - s.center;
    let a = dot(rd, rd);
    let b = 2.0 * dot(oc, rd);
    let c = dot(oc, oc) - s.radius * s.radius;
    let disc = b*b - 4.0*a*c;
    if disc < 0.0 { return 1e9; }
    let t = (-b - sqrt(disc)) / (2.0 * a);
    return select(1e9, t, t > 0.0);
}

fn intersect_plane(ro: vec3<f32>, rd: vec3<f32>, p: Plane) -> f32 {
    let denom = dot(p.normal, rd);
    if abs(denom) < 1e-6 { return 1e9; }
    let t = dot(p.point - ro, p.normal) / denom;
    return select(1e9, t, t > 0.0);
}

fn shade(hit: vec3<f32>, normal: vec3<f32>, color: vec3<f32>) -> vec3<f32> {
    let l = normalize(light.pos - hit);
    let diff = max(dot(normal, l), 0.0);
    let dist = distance(light.pos, hit);
    let dist2 = dist * dist;
    return color * diff * light.intensity / dist2;
}

@compute @workgroup_size(8,8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= camera.width || gid.y >= camera.height { return; }
    let width = f32(camera.width);
    let height = f32(camera.height);
    let aspect = width / height;
    let scale = tan(camera.fov * 0.5);
    let forward = normalize(camera.look_at - camera.pos);
    let right = normalize(cross(forward, camera.up));
    let up = cross(right, forward);
    let u = (f32(gid.x) / width - 0.5) * 2.0 * aspect * scale;
    let v = -(f32(gid.y) / height - 0.5) * 2.0 * scale;
    var dir = normalize(u*right + v*up + forward);

    var t_min = 1e9;
    var hit_normal = vec3<f32>(0.0,0.0,0.0);
    var hit_color = vec3<f32>(0.0,0.0,0.0);

    for (var i=0u; i<camera.sphere_count; i=i+1u) {
        let s = spheres[i];
        let t = intersect_sphere(camera.pos, dir, s);
        if t < t_min {
            t_min = t;
            let hit = camera.pos + dir * t;
            hit_normal = normalize(hit - s.center);
            hit_color = s.color;
        }
    }
    for (var i=0u; i<camera.plane_count; i=i+1u) {
        let p = planes[i];
        let t = intersect_plane(camera.pos, dir, p);
        if t < t_min {
            t_min = t;
            hit_normal = p.normal;
            hit_color = p.color;
        }
    }

    var out_color = vec3<f32>(0.0,0.0,0.0);
    if t_min < 1e9 {
        let hit = camera.pos + dir * t_min;
        out_color = shade(hit, hit_normal, hit_color);
    }
    let r = u32(clamp(out_color.x, 0.0, 1.0) * 255.0);
    let g = u32(clamp(out_color.y, 0.0, 1.0) * 255.0);
    let b = u32(clamp(out_color.z, 0.0, 1.0) * 255.0);
    let index = gid.y * camera.width + gid.x;
    output[index] = r | (g<<8u) | (b<<16u) | 0xff000000u;
}
