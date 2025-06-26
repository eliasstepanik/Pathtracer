use crate::{scene::Scene, object::Object};
use wgpu::util::DeviceExt;
use image::{ImageBuffer, RgbaImage};
use bytemuck::{Pod, Zeroable};
use rand::Rng;

pub fn render(scene: &Scene) -> RgbaImage {
    pollster::block_on(render_async(scene))
}

async fn render_async(scene: &Scene) -> RgbaImage {
    // Setup wgpu
    let instance = wgpu::Instance::default();
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }).await.expect("Failed to find GPU adapter");
    let (device, queue) = adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("High Limits Device"),
            features: wgpu::Features::empty(), // Use default features
            // This is the key part: we are overriding the default conservative limits
            // with the actual maximums supported by the hardware.
            limits: adapter.limits(),
        },
        None,
    ).await.expect("Failed to create device");

    let width = scene.render.width;
    let height = scene.render.height;

    // --- Prepare data structs ---
    const MAX_SPHERES: usize = 32;
    const MAX_PLANES: usize = 32;

    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    struct CameraUniform {
        pos: [f32; 4],
        look_at: [f32; 4],
        up: [f32; 4],
        forward: [f32; 4],
        width: u32,
        height: u32,
        fov: f32,
        sphere_count: u32,
        plane_count: u32,
        aperture: f32,
        focus_dist: f32,
        _pad4: u32, // This last pad aligns the total struct size.
    }

    // NEW: RenderParams to hold sampling info
    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    struct RenderParams {
        samples_per_pixel: u32,
        max_bounces: u32,
        seed1: u32,
        seed2: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    struct LightUniform {
        pos: [f32; 4], // MODIFIED
        intensity: [f32; 4], // MODIFIED
        u: [f32; 4], // MODIFIED
        v: [f32; 4], // MODIFIED
    }

    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    struct SphereData {
        center: [f32; 4], // MODIFIED
        color: [f32; 4], // MODIFIED
        radius: f32,
        metallic: f32,
        roughness: f32,
        _pad: f32, // MODIFIED to a single f32 for alignment
    }

    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    struct PlaneData {
        point: [f32; 4], // MODIFIED
        normal: [f32; 4], // MODIFIED
        u: [f32; 4], // MODIFIED
        v: [f32; 4], // MODIFIED
        color: [f32; 4], // MODIFIED
        metallic: f32,
        roughness: f32,
        _pad: [f32; 2], // MODIFIED for alignment
    }
    let forward = (scene.camera.look_at - scene.camera.pos).normalize();
    let focus_dist = crate::renderer::autofocus(scene.camera.pos, forward.cross(scene.camera.up).normalize(), forward.cross(forward.cross(scene.camera.up).normalize()), forward, scene.render.width as f32 / scene.render.height as f32, (scene.camera.fov.to_radians() * 0.5).tan(), width, height, &scene.objects);

    let cam = CameraUniform {
        pos: [scene.camera.pos.0, scene.camera.pos.1, scene.camera.pos.2, 0.0],
        look_at: [scene.camera.look_at.0, scene.camera.look_at.1, scene.camera.look_at.2, 0.0],
        up: [scene.camera.up.0, scene.camera.up.1, scene.camera.up.2, 0.0],
        forward: [forward.0, forward.1, forward.2, 0.0],
        width, height,
        fov: scene.camera.fov,
        sphere_count: scene.objects.iter().filter(|o| matches!(o, Object::Sphere(_))).count() as u32,
        plane_count: scene.objects.iter().filter(|o| matches!(o, Object::Plane(_))).count() as u32,
        aperture: scene.camera.aperture,
        focus_dist,
        _pad4: 0,
    };

    let mut rng = rand::thread_rng();
    let params = RenderParams {
        samples_per_pixel: scene.render.samples,
        max_bounces: 5,
        seed1: rng.r#gen(),
        seed2: rng.r#gen(),
    };

    let light = scene.lights.get(0).expect("Scene needs at least one light");
    let light_uniform = LightUniform {
        pos: [light.pos.0, light.pos.1, light.pos.2, 0.0], // MODIFIED
        intensity: [light.intensity.0, light.intensity.1, light.intensity.2, 0.0], // MODIFIED
        u: [light.u.0, light.u.1, light.u.2, 0.0], // MODIFIED
        v: [light.v.0, light.v.1, light.v.2, 0.0], // MODIFIED
    };

    let mut spheres = vec![SphereData::zeroed(); MAX_SPHERES];
    let mut planes = vec![PlaneData::zeroed(); MAX_PLANES];
    let mut scount = 0;
    let mut pcount = 0;
    for obj in &scene.objects {
        match obj {
            Object::Sphere(s) if scount < MAX_SPHERES => {
                spheres[scount] = SphereData {
                    center: [s.center.0, s.center.1, s.center.2, 0.0], // MODIFIED
                    color: [s.material.color.0, s.material.color.1, s.material.color.2, 0.0], // MODIFIED
                    radius: s.radius,
                    metallic: s.material.metallic,
                    roughness: s.material.roughness,
                    _pad: 0.0,
                };
                scount += 1;
            }
            Object::Plane(p) if pcount < MAX_PLANES => {
                planes[pcount] = PlaneData {
                    point: [p.point.0, p.point.1, p.point.2, 0.0], // MODIFIED
                    normal: [p.normal.0, p.normal.1, p.normal.2, 0.0], // MODIFIED
                    u: [p.u.0, p.u.1, p.u.2, 0.0], // MODIFIED
                    v: [p.v.0, p.v.1, p.v.2, 0.0], // MODIFIED
                    color: [p.material.color.0, p.material.color.1, p.material.color.2, 0.0], // MODIFIED
                    metallic: p.material.metallic,
                    roughness: p.material.roughness,
                    _pad: [0.0; 2],
                };
                pcount += 1;
            }
            _ => {}
        }
    }

    // GPU buffers
    let cam_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("Camera"), contents: bytemuck::bytes_of(&cam), usage: wgpu::BufferUsages::UNIFORM });
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("Params"), contents: bytemuck::bytes_of(&params), usage: wgpu::BufferUsages::UNIFORM });
    let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("Light"), contents: bytemuck::bytes_of(&light_uniform), usage: wgpu::BufferUsages::UNIFORM });
    let sphere_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("Spheres"), contents: bytemuck::cast_slice(&spheres), usage: wgpu::BufferUsages::STORAGE });
    let plane_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("Planes"), contents: bytemuck::cast_slice(&planes), usage: wgpu::BufferUsages::STORAGE });

    let output_buffer_size = (width * height * 4) as wgpu::BufferAddress;
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("Output"), size: output_buffer_size, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("Staging"), size: output_buffer_size, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("Pathtrace"), source: wgpu::ShaderSource::Wgsl(include_str!("gpu_pathtrace.wgsl").into())});

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bind group layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry { binding:0, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer{ ty:wgpu::BufferBindingType::Uniform, has_dynamic_offset:false, min_binding_size: None }, count: None },
            wgpu::BindGroupLayoutEntry { binding:1, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer{ ty:wgpu::BufferBindingType::Uniform, has_dynamic_offset:false, min_binding_size: None }, count: None },
            wgpu::BindGroupLayoutEntry { binding:2, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer{ ty:wgpu::BufferBindingType::Uniform, has_dynamic_offset:false, min_binding_size: None }, count: None },
            wgpu::BindGroupLayoutEntry { binding:3, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer{ ty:wgpu::BufferBindingType::Storage { read_only:true }, has_dynamic_offset:false, min_binding_size: None }, count: None },
            wgpu::BindGroupLayoutEntry { binding:4, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer{ ty:wgpu::BufferBindingType::Storage { read_only:true }, has_dynamic_offset:false, min_binding_size: None }, count: None },
            wgpu::BindGroupLayoutEntry { binding:5, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer{ ty:wgpu::BufferBindingType::Storage { read_only:false }, has_dynamic_offset:false, min_binding_size: None }, count: None },
        ],
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bind group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry { binding:0, resource: cam_buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding:1, resource: params_buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding:2, resource: light_buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding:3, resource: sphere_buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding:4, resource: plane_buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding:5, resource: output_buffer.as_entire_binding() },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("pipeline layout"), bind_group_layouts: &[&bind_group_layout], push_constant_ranges: &[]});
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("pipeline"), layout: Some(&pipeline_layout), module: &shader, entry_point: "main" });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("encoder") });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("compute pass") });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        let (wgx, wgy) = (8u32, 8u32);
        cpass.dispatch_workgroups((width + wgx -1)/wgx, (height + wgy -1)/wgy, 1);
    }
    encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_buffer_size);
    queue.submit(Some(encoder.finish()));


    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = futures_intrusive::channel::shared::oneshot_channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());
    device.poll(wgpu::Maintain::Wait);
    rx.receive().await.unwrap().expect("map failed");

    let data = buffer_slice.get_mapped_range();
    let mut img = RgbaImage::new(width, height);
    let pixels: &[u32] = bytemuck::cast_slice(&data);

    for (i, pixel) in pixels.iter().enumerate() {
        let x = (i as u32) % width;
        let y = (i as u32) / width;

        // Unpack the u32 color into R, G, B, A bytes
        let r = (*pixel & 0xff) as u8;
        let g = ((*pixel >> 8) & 0xff) as u8;
        let b = ((*pixel >> 16) & 0xff) as u8;
        let a = ((*pixel >> 24) & 0xff) as u8;

        img.put_pixel(x, y, image::Rgba([r, g, b, a]));
    }

    drop(data);
    staging_buffer.unmap();
    img
}