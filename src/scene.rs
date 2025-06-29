use serde::Deserialize;
use std::collections::HashMap;
use crate::{algebra::Vec3, material::Material, plane::Plane, sphere::Sphere, light::Light, algebra::vec3_from_array, sphere, plane};
use crate::object::Object;

#[derive(Deserialize)]
pub struct CameraJson {
    #[serde(deserialize_with = "vec3_from_array")]
    pub pos:      Vec3,
    #[serde(deserialize_with = "vec3_from_array")]
    pub look_at:  Vec3,
    #[serde(deserialize_with = "vec3_from_array")]
    pub up:       Vec3,
    pub fov:      f32,
    pub aperture: f32,
}
#[derive(Deserialize)]
pub struct RenderJson { pub width:u32, pub height:u32, pub samples:u32 }

#[derive(Deserialize)] struct MaterialJson {
    rgb:[f32;3],
    metallic:f32,
    roughness:f32,
    ior:f32,
    #[serde(default)] // If missing in JSON, it will use the default value (0.0)
    volume_density: f32,
    #[serde(default)]
    volume_anisotropy: f32,
}


#[derive(Deserialize)]
#[serde(untagged)]
enum ObjectJson {
    Sphere{ sphere: SphereDesc },
    Plane { plane : PlaneDesc  },
}

#[derive(Deserialize)]
pub struct SphereDesc {
    pub name:   String,
    #[serde(deserialize_with = "vec3_from_array")]
    pub center: Vec3,
    pub radius: f32,
    pub mat:    String,
    #[serde(default)] // Default to false if not present in JSON
    pub in_focus: bool,
}
#[derive(Deserialize)]
pub struct PlaneDesc {
    pub name:   String,
    #[serde(deserialize_with = "vec3_from_array")]
    pub point : Vec3,
    #[serde(deserialize_with = "vec3_from_array")]
    pub u:      Vec3,
    #[serde(deserialize_with = "vec3_from_array")]
    pub v:      Vec3,
    pub mat   : String,
    #[serde(default)] // Default to false if not present in JSON
    pub in_focus: bool,
}


#[derive(Deserialize)]
pub struct LightJson {
    #[serde(deserialize_with = "vec3_from_array")]
    pub pos:       Vec3,
    #[serde(deserialize_with = "vec3_from_array")]
    pub u:         Vec3,
    #[serde(deserialize_with = "vec3_from_array")]
    pub v:         Vec3,
    #[serde(deserialize_with = "vec3_from_array")]
    pub intensity: Vec3,
}


#[derive(Deserialize)]
struct SceneFile {
    camera   : CameraJson,
    render   : RenderJson,
    materials: HashMap<String, MaterialJson>,
    objects  : Vec<ObjectJson>,
    lights   : Vec<LightJson>,
}

/// Public “loaded” scene
pub struct Scene {
    pub camera : CameraJson,
    pub render : RenderJson,
    pub objects: Vec<crate::object::Object>,
    pub lights : Vec<Light>,
}

pub fn load(path:&str) -> Scene {
    let data = std::fs::read_to_string(path).expect("scene file");
    let file : SceneFile = serde_json::from_str(&data).expect("json parse");

    // 1. Create a library of materials from the JSON
    let materials: HashMap<String, Material> = file.materials.into_iter().map(|(name, m)| {
        let mat = Material {
            color: Vec3(m.rgb[0], m.rgb[1], m.rgb[2]),
            metallic: m.metallic,
            roughness: m.roughness,
            ior: m.ior,
            // --- NEW: Assign volume properties ---
            volume_density: m.volume_density,
            volume_anisotropy: m.volume_anisotropy,
        };
        (name, mat)
    }).collect();

    let default_mat = Material {
        color: Vec3(1.0, 0.0, 1.0),
        metallic: 0.0,
        roughness: 1.0,
        ior: 1.0,
        // --- NEW ---
        volume_density: 0.0,
        volume_anisotropy: 0.0
    };


    // 2. Create objects and assign materials from the library by name
    let mut objects = Vec::new();
    for o in file.objects {
        match o {
            ObjectJson::Sphere { sphere } => {
                let material = *materials.get(&sphere.mat).unwrap_or(&default_mat);
                objects.push(Object::Sphere(Sphere {
                    name:     sphere.name,
                    center:   sphere.center,
                    radius:   sphere.radius,
                    material,
                    in_focus: sphere.in_focus, // ADDED
                }));
            },
            ObjectJson::Plane { plane } => {
                let material = *materials.get(&plane.mat).unwrap_or(&default_mat);
                let normal = plane.u.cross(plane.v).normalize();
                objects.push(Object::Plane(Plane {
                    name:     plane.name,
                    point:    plane.point,
                    u:        plane.u,
                    v:        plane.v,
                    normal,
                    material,
                    in_focus: plane.in_focus, // ADDED
                }));
            }
        }
    }

    let lights = file.lights.iter().map(|l| Light{
        pos:l.pos, u:l.u, v:l.v, intensity:l.intensity
    }).collect();

    Scene{ camera:file.camera, render:file.render, objects, lights }
}