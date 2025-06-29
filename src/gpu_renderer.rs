// C:\Users\Elias Stepanik\RustroverProjects\Pathtracer\src\gpu_renderer.rs

use crate::{object::Object, scene::Scene};
use bytemuck::{Pod, Zeroable};
use image::RgbaImage;
use rand::Rng;
use wgpu::util::DeviceExt;
use wgpu::DeviceType;

// The public-facing function signature must now be mutable to allow updating the scene's internal state if needed.
// For now, we only read from it, but this is good practice for future features.
pub fn render(scene: &Scene) -> RgbaImage {
    pollster::block_on(render_async(scene))
}

// All structs are defined once at the top for clarity.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    pos: [f32; 4],
    forward: [f32; 4],
    up: [f32; 4],
    right: [f32; 4],
    width: u32,
    height: u32,
    fov: f32,
    sphere_count: u32,
    plane_count: u32,
    triangle_count: u32,
    aperture: f32,
    focus_dist: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RenderParams {
    samples_per_pixel: u32,
    max_bounces: u32,
    seed1: u32,
    seed2: u32,
}

fn detect_gpu_workload(adapter: &wgpu::Adapter) -> u64 {
    match adapter.get_info().device_type {
        DeviceType::Cpu => 10_000_000,
        _ => 40_000_000,
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LightUniform {
    pos: [f32; 4],
    intensity: [f32; 4],
    u: [f32; 4],
    v: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SphereData {
    center: [f32; 4],
    color: [f32; 4],
    radius: f32,
    metallic: f32,
    roughness: f32,
    ior: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PlaneData {
    point: [f32; 4],
    normal: [f32; 4],
    u: [f32; 4],
    v: [f32; 4],
    color: [f32; 4],
    metallic: f32,
    roughness: f32,
    ior: f32,
    _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TriangleData {
    v0: [f32; 4],
    v1: [f32; 4],
    v2: [f32; 4],
    normal: [f32; 4],
    color: [f32; 4],
    metallic: f32,
    roughness: f32,
    ior: f32,
    _pad: f32,
}

async fn render_async(scene: &Scene) -> RgbaImage {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .expect("Failed to find GPU adapter");
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Device"),
                features: wgpu::Features::empty(),
                limits: adapter.limits(),
            },
            None,
        )
        .await
        .expect("Failed to create device");

    let width = scene.render.width;
    let height = scene.render.height;

    // --- Progressive Render Setup ---
    let total_samples = scene.render.samples;

    let target_workload_per_dispatch: u64 = scene
        .render
        .gpu_workload
        .unwrap_or_else(|| detect_gpu_workload(&adapter));
    println!("Target workload per dispatch: {}", target_workload_per_dispatch);
    let pixels = (width * height) as u64;

    let mut samples_per_dispatch = (target_workload_per_dispatch / pixels.max(1)) as u32;
    samples_per_dispatch = samples_per_dispatch.max(1).min(total_samples);

    let num_dispatches = (total_samples + samples_per_dispatch - 1) / samples_per_dispatch;

    let mut accumulated_color = vec![[0.0f32; 4]; (width * height) as usize];
    let mut rng = rand::thread_rng();

    println!("Starting progressive render: {} dispatches of {} samples each for a total of {} samples/pixel.", num_dispatches, samples_per_dispatch, total_samples);

    // --- Pre-computation ---
    let forward = (scene.camera.look_at - scene.camera.pos).normalize();
    let right = scene.camera.up.cross(forward).normalize();
    let up = right.cross(forward);
    let focus_dist = crate::renderer::autofocus(
        scene.camera.pos,
        right,
        up,
        forward,
        width as f32 / height as f32,
        (scene.camera.fov.to_radians() * 0.5).tan(),
        width,
        height,
        &scene.objects,
    );

    let light = scene.lights.get(0).expect("Scene needs at least one light");
    let light_uniform = LightUniform {
        pos: [light.pos.0, light.pos.1, light.pos.2, 0.0],
        intensity: [light.intensity.0, light.intensity.1, light.intensity.2, 0.0],
        u: [light.u.0, light.u.1, light.u.2, 0.0],
        v: [light.v.0, light.v.1, light.v.2, 0.0],
    };

    let (spheres, planes, tris, sphere_count, plane_count, tri_count) =
        get_object_data(scene);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Pathtrace Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("gpu_pathtrace.wgsl").into()),
    });
    let pipeline = create_compute_pipeline(&device, &shader);

    // --- Progressive Render Loop ---
    for i in 0..num_dispatches {
        let params = RenderParams {
            samples_per_pixel: samples_per_dispatch,
            max_bounces: 12,
            seed1: rng.gen(),
            seed2: rng.gen(),
        };

        let cam = CameraUniform {
            pos: [
                scene.camera.pos.0,
                scene.camera.pos.1,
                scene.camera.pos.2,
                0.0,
            ],
            forward: [forward.0, forward.1, forward.2, 0.0],
            up: [up.0, up.1, up.2, 0.0], // Send the correct up vector
            right: [right.0, right.1, right.2, 0.0],
            width,
            height,
            fov: scene.camera.fov,
            sphere_count,
            plane_count,
            triangle_count: tri_count,
            aperture: scene.camera.aperture,
            focus_dist,
        };

        // --- START: BUG FIX ---
        // Instead of a flawed helper trait, we create the resources and hold onto
        // the output_buffer directly.
        let (bind_group, staging_buffer, output_buffer, output_buffer_size) =
            create_dispatch_resources(
                &device,
                &pipeline,
                &cam,
                &params,
                &light_uniform,
                &spheres,
                &planes,
                &tris,
            );

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Encoder"),
        });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute Pass"),
            });
            cpass.set_pipeline(&pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups((width + 7) / 8, (height + 7) / 8, 1);
        }
        // Now we use our direct reference to the output_buffer.
        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_buffer_size);
        // --- END: BUG FIX ---

        queue.submit(Some(encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = futures_intrusive::channel::shared::oneshot_channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());
        device.poll(wgpu::Maintain::Wait);
        rx.receive().await.unwrap().expect("map failed");

        let data = buffer_slice.get_mapped_range();
        let pixels: &[[f32; 4]] = bytemuck::cast_slice(&data);
        for (j, pixel_color) in pixels.iter().enumerate() {
            accumulated_color[j][0] += pixel_color[0];
            accumulated_color[j][1] += pixel_color[1];
            accumulated_color[j][2] += pixel_color[2];
        }
        drop(data);
        staging_buffer.unmap();
        println!("Dispatch {}/{} complete.", i + 1, num_dispatches);
    }

    // --- Final Image Creation ---
    let mut img = RgbaImage::new(width, height);
    for (i, pixel_data) in accumulated_color.iter().enumerate() {
        let x = (i as u32) % width;
        let y = height - 1 - (i as u32) / width;
        let avg_r = pixel_data[0] / total_samples as f32;
        let avg_g = pixel_data[1] / total_samples as f32;
        let avg_b = pixel_data[2] / total_samples as f32;
        let tonemapped = crate::tonemap::aces_film(crate::algebra::Vec3(avg_r, avg_g, avg_b));
        let r = (tonemapped.0.powf(1.0 / 2.2) * 255.0).min(255.0) as u8;
        let g = (tonemapped.1.powf(1.0 / 2.2) * 255.0).min(255.0) as u8;
        let b = (tonemapped.2.powf(1.0 / 2.2) * 255.0).min(255.0) as u8;
        img.put_pixel(x, y, image::Rgba([r, g, b, 255]));
    }
    img
}

// Helper function to keep the main loop cleaner by setting up buffers.
fn get_object_data(scene: &Scene) -> (
    Vec<SphereData>,
    Vec<PlaneData>,
    Vec<TriangleData>,
    u32,
    u32,
    u32,
) {
    const MAX_SPHERES: usize = 32;
    const MAX_PLANES: usize = 32;
    const MAX_TRIS: usize = 8192;
    let mut spheres = vec![SphereData::zeroed(); MAX_SPHERES];
    let mut planes = vec![PlaneData::zeroed(); MAX_PLANES];
    let mut tris = vec![TriangleData::zeroed(); MAX_TRIS];
    let (mut scount, mut pcount, mut tcount) = (0, 0, 0);
    for obj in &scene.objects {
        match obj {
            Object::Sphere(s) if scount < MAX_SPHERES => {
                spheres[scount] = SphereData {
                    center: [s.center.0, s.center.1, s.center.2, 0.0],
                    color: [
                        s.material.color.0,
                        s.material.color.1,
                        s.material.color.2,
                        0.0,
                    ],
                    radius: s.radius,
                    metallic: s.material.metallic,
                    roughness: s.material.roughness,
                    ior: s.material.ior,
                };
                scount += 1;
            }
            Object::Plane(p) if pcount < MAX_PLANES => {
                planes[pcount] = PlaneData {
                    point: [p.point.0, p.point.1, p.point.2, 0.0],
                    normal: [p.normal.0, p.normal.1, p.normal.2, 0.0],
                    u: [p.u.0, p.u.1, p.u.2, 0.0],
                    v: [p.v.0, p.v.1, p.v.2, 0.0],
                    color: [
                        p.material.color.0,
                        p.material.color.1,
                        p.material.color.2,
                        0.0,
                    ],
                    metallic: p.material.metallic,
                    roughness: p.material.roughness,
                    ior: p.material.ior,
                    _pad: 0.0,
                };
                pcount += 1;
            }
            Object::Mesh(m) => {
                for tri in &m.triangles {
                    if tcount >= MAX_TRIS {
                        break;
                    }
                    tris[tcount] = TriangleData {
                        v0: [tri.v0.0, tri.v0.1, tri.v0.2, 0.0],
                        v1: [tri.v1.0, tri.v1.1, tri.v1.2, 0.0],
                        v2: [tri.v2.0, tri.v2.1, tri.v2.2, 0.0],
                        normal: [tri.normal.0, tri.normal.1, tri.normal.2, 0.0],
                        color: [
                            m.material.color.0,
                            m.material.color.1,
                            m.material.color.2,
                            0.0,
                        ],
                        metallic: m.material.metallic,
                        roughness: m.material.roughness,
                        ior: m.material.ior,
                        _pad: 0.0,
                    };
                    tcount += 1;
                }
            }
            _ => {}
        }
    }
    spheres.truncate(scount);
    planes.truncate(pcount);
    tris.truncate(tcount);

    if spheres.is_empty() {
        spheres.push(SphereData::zeroed());
    }
    if planes.is_empty() {
        planes.push(PlaneData::zeroed());
    }
    if tris.is_empty() {
        tris.push(TriangleData::zeroed());
    }
    (
        spheres,
        planes,
        tris,
        scount as u32,
        pcount as u32,
        tcount as u32,
    )
}

// Helper to create the compute pipeline
fn create_compute_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
) -> wgpu::ComputePipeline {
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 6,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: "main",
    })
}

// Helper to create resources for a single dispatch
fn create_dispatch_resources(
    device: &wgpu::Device,
    pipeline: &wgpu::ComputePipeline,
    cam: &CameraUniform,
    params: &RenderParams,
    light_uniform: &LightUniform,
    spheres: &[SphereData],
    planes: &[PlaneData],
    triangles: &[TriangleData],
) -> (wgpu::BindGroup, wgpu::Buffer, wgpu::Buffer, u64) {
    let cam_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Camera"),
        contents: bytemuck::bytes_of(cam),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Params"),
        contents: bytemuck::bytes_of(params),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Light"),
        contents: bytemuck::bytes_of(light_uniform),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let sphere_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Spheres"),
        contents: bytemuck::cast_slice(spheres),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let plane_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Planes"),
        contents: bytemuck::cast_slice(planes),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let tri_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Triangles"),
        contents: bytemuck::cast_slice(triangles),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let output_buffer_size = (cam.width * cam.height * 16) as wgpu::BufferAddress;
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output"),
        size: output_buffer_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Staging"),
        size: output_buffer_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Bind Group"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: cam_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: params_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: light_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: sphere_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: plane_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: tri_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: output_buffer.as_entire_binding(),
            },
        ],
    });

    (
        bind_group,
        staging_buffer,
        output_buffer,
        output_buffer_size,
    )
}
