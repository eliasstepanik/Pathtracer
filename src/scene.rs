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

#[derive(Deserialize)] struct MaterialJson{ rgb:[f32;3], metallic:f32, roughness:f32, ior:f32 }

#[derive(Deserialize)]
#[serde(untagged)]
enum ObjectJson {
    Sphere{ sphere: SphereDesc },
    Plane { plane : PlaneDesc  },
}
#[derive(Deserialize)]
pub struct SphereDesc {
    #[serde(deserialize_with = "vec3_from_array")]
    pub center: Vec3,
    pub radius: f32,
    pub mat:    String,
}
#[derive(Deserialize)]
pub struct PlaneDesc {
    #[serde(deserialize_with = "vec3_from_array")]
    pub point : Vec3,
    #[serde(deserialize_with = "vec3_from_array")]
    pub normal: Vec3,
    #[serde(default)]                    // <— allow missing field
    pub size  : Option<[f32; 2]>,        // width , height
    pub mat   : String,
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
    pub objects: Vec<crate::object::Object>,  // enum wrapping Plane|Sphere
    pub lights : Vec<Light>,
}

pub fn load(path:&str) -> Scene {
    let data = std::fs::read_to_string(path).expect("scene file");
    let file : SceneFile = serde_json::from_str(&data).expect("json parse");

    let mat_of = |name:&str| -> Material {
        let m = file.materials.get(name).unwrap_or_else(||panic!("no material {}",name));
        Material{
            color:Vec3(m.rgb[0],m.rgb[1],m.rgb[2]),
            metallic:m.metallic, roughness:m.roughness, ior:m.ior }
    };

    let mut objects = Vec::new();
    for o in file.objects {
        match o {
            ObjectJson::Sphere { sphere } => objects.push(Object::Sphere(Sphere {
                center:   sphere.center,
                radius:   sphere.radius,
                material: mat_of(&sphere.mat),
            })),
            ObjectJson::Plane { plane } => {
                let [w, h] = plane.size.unwrap_or([f32::INFINITY, f32::INFINITY]);
                objects.push(Object::Plane(Plane {
                    point   : plane.point,
                    normal  : plane.normal.normalize(),
                    half_w  : w * 0.5,
                    half_h  : h * 0.5,
                    material: mat_of(&plane.mat),
                }));
            }
        }
    }


    let lights = file.lights.iter().map(|l| Light{
        pos:l.pos.into(), u:l.u.into(), v:l.v.into(), intensity:l.intensity.into()
    }).collect();

    Scene{ camera:file.camera, render:file.render, objects, lights }
}
