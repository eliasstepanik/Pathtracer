bl_info = {
    "name": "Rust Pathtracer Scene Builder",
    "author": "Elias Stepanik (Rewritten by AI)",
    "description": "A robust tool to create, import, and export scenes for the Rust pathtracer.",
    "version": (3, 1, 0), # Major architectural fix for Import
    "blender": (4, 1, 0),
    "location": "3D View > Sidebar > Ray Scene",
    "category": "Import-Export",
    "doc_url": "https://github.com/elias-stepanik/pathtracer",
}

import bpy
import json
import math
import mathutils as mu
from pathlib import Path
from bpy.props import (FloatProperty, IntProperty, StringProperty,
                       PointerProperty)
from bpy.types import Operator, Panel, PropertyGroup, Context, Material, Scene


# ───────────────────────── CORE TRANSFORMATION LOGIC ──────────────────────────

# This matrix is the key to the entire export process. It performs the
# fundamental coordinate system swap from Blender's Z-Up to the Pathtracer's Y-Up.
# Blender (X, Y, Z) -> Pathtracer (X, Z, -Y)
CONVERSION_MATRIX_BLENDER_TO_PATHTRACER = mu.Matrix((
    (1, 0, 0, 0),
    (0, 0, 1, 0),
    (0, -1, 0, 0),
    (0, 0, 0, 1)
))

# Custom property key used to identify objects managed by this addon
PATHTRACER_OBJECT_ID_KEY = "rs_type"

def vector_to_list(vector: mu.Vector) -> list[float]:
    """A simple utility to convert a mathutils Vector to a basic list."""
    return [vector.x, vector.y, vector.z]


def create_look_at_quaternion(direction: mu.Vector, up: mu.Vector = mu.Vector((0, 1, 0))) -> mu.Quaternion:
    """Creates a quaternion that orients an object to look in a specific direction."""
    z_axis = -direction.normalized()
    if abs(z_axis.dot(up)) > 0.99999:
        up = mu.Vector((1, 0, 0)) if abs(z_axis.x) < 0.99999 else mu.Vector((0, 1, 0))

    x_axis = up.cross(z_axis).normalized()
    y_axis = z_axis.cross(x_axis).normalized()
    return mu.Matrix((x_axis, y_axis, z_axis)).transposed().to_quaternion()


# ───────────────────────── PROPERTY DEFINITIONS ──────────────────────────

class PathtracerMaterialProperties(PropertyGroup):
    metallic: FloatProperty(name="Metallic", min=0, max=1, default=0)
    roughness: FloatProperty(name="Roughness", min=0.01, max=1, default=0.5)
    ior: FloatProperty(name="Index of Refraction", min=1, max=3, default=1.5)
    volume_density: FloatProperty(name="Volume Density", min=0, max=10, default=0)
    volume_anisotropy: FloatProperty(name="Volume Anisotropy", min=-1, max=1, default=0)


class PathtracerSceneProperties(PropertyGroup):
    samples: IntProperty(name="Samples", min=1, max=65536, default=128)
    aperture: FloatProperty(name="Aperture", min=0, max=1, default=0.01, precision=4)


# ───────────────────────── PRIMITIVE CREATION OPERATORS ────────────────────────────

class RS_OT_AddPlane(Operator):
    bl_idname = "rs.add_plane"; bl_label = "Add Pathtracer Plane"; bl_options = {'REGISTER', 'UNDO'}
    def execute(self, context: Context):
        bpy.ops.mesh.primitive_plane_add(size=2, location=context.scene.cursor.location)
        context.active_object[PATHTRACER_OBJECT_ID_KEY] = "plane"
        return {'FINISHED'}


class RS_OT_AddSphere(Operator):
    bl_idname = "rs.add_sphere"; bl_label = "Add Pathtracer Sphere"; bl_options = {'REGISTER', 'UNDO'}
    def execute(self, context: Context):
        bpy.ops.mesh.primitive_uv_sphere_add(radius=1, location=context.scene.cursor.location)
        context.active_object[PATHTRACER_OBJECT_ID_KEY] = "sphere"
        return {'FINISHED'}


class RS_OT_AddLight(Operator):
    bl_idname = "rs.add_light"; bl_label = "Add Pathtracer Light"; bl_options = {'REGISTER', 'UNDO'}
    size_x: FloatProperty(name="Width", default=2, min=0.01)
    size_y: FloatProperty(name="Height", default=2, min=0.01)
    energy: FloatProperty(name="Intensity", default=50, min=0)
    def execute(self, context: Context):
        bpy.ops.object.light_add(type='AREA', location=context.scene.cursor.location)
        light_object = context.active_object
        blender_light_data = light_object.data
        blender_light_data.shape = 'RECTANGLE'
        blender_light_data.size = self.size_x
        blender_light_data.size_y = self.size_y
        blender_light_data.energy = self.energy
        light_object.name = "RS_AreaLight"
        return {'FINISHED'}


# ───────────────────────── SCENE EXPORT OPERATOR ──────────────────────────────────

class RS_OT_ExportScene(Operator):
    bl_idname = "rs.export_scene"; bl_label = "Export to scene.json"
    filepath: StringProperty(subtype='FILE_PATH', default="scene.json")

    def execute(self, context: Context) -> set:
        blender_scene = context.scene
        used_materials = {
            obj.active_material for obj in blender_scene.objects
            if PATHTRACER_OBJECT_ID_KEY in obj and obj.active_material
        }
        materials_as_json = self._serialize_materials(used_materials)
        objects_as_json = self._serialize_objects(blender_scene)
        lights_as_json = self._serialize_lights(blender_scene)

        if not blender_scene.camera:
            self.report({'ERROR'}, "No active camera in the scene."); return {'CANCELLED'}
        camera_as_json = self._serialize_camera(blender_scene.camera, blender_scene.rs_props.aperture)
        render_as_json = self._serialize_render_settings(blender_scene)

        final_scene_data = {
            "camera": camera_as_json, "render": render_as_json, "materials": materials_as_json,
            "objects": objects_as_json, "lights": lights_as_json,
        }
        try:
            with open(self.filepath, "w") as json_file:
                json.dump(final_scene_data, json_file, indent=4)
        except Exception as error:
            self.report({'ERROR'}, f"Failed to write file: {error}"); return {'CANCELLED'}

        self.report({'INFO'}, f"Scene exported successfully to {self.filepath}"); return {'FINISHED'}

    def invoke(self, context: Context, event):
        context.window_manager.fileselect_add(self); return {'RUNNING_MODAL'}

    def _serialize_materials(self, materials: set) -> dict:
        materials_data = {}
        for material in materials:
            props = material.rs_props
            materials_data[material.name] = {
                "rgb": list(material.diffuse_color)[:3], "metallic": props.metallic,
                "roughness": props.roughness, "ior": props.ior,
                "volume_density": props.volume_density, "volume_anisotropy": props.volume_anisotropy
            }
        return materials_data

    def _serialize_objects(self, blender_scene: Scene) -> list:
        objects_data = []
        for blender_object in blender_scene.objects:
            if PATHTRACER_OBJECT_ID_KEY not in blender_object or not blender_object.active_material:
                continue

            final_transform = CONVERSION_MATRIX_BLENDER_TO_PATHTRACER @ blender_object.matrix_world
            common_properties = {"name": blender_object.name, "mat": blender_object.active_material.name}
            pathtracer_object_type = blender_object[PATHTRACER_OBJECT_ID_KEY]

            if pathtracer_object_type == "sphere":
                center_pathtracer = final_transform.translation
                radius = blender_object.dimensions.x / 2.0
                objects_data.append({"sphere": {
                    **common_properties,
                    "center": vector_to_list(center_pathtracer), "radius": radius
                }})

            elif pathtracer_object_type == "plane":
                dims = blender_object.dimensions
                half_width = dims.x / 2.0
                half_height = dims.y / 2.0
                center_local = mu.Vector((0, 0, 0))
                u_edge_local = mu.Vector((half_width, 0, 0))
                v_edge_local = mu.Vector((0, half_height, 0))
                center_pathtracer = final_transform @ center_local
                u_edge_pathtracer = final_transform @ u_edge_local
                v_edge_pathtracer = final_transform @ v_edge_local
                u_vector = u_edge_pathtracer - center_pathtracer
                v_vector = v_edge_pathtracer - center_pathtracer
                objects_data.append({"plane": {
                    **common_properties,
                    "point": vector_to_list(center_pathtracer),
                    "u": vector_to_list(u_vector), "v": vector_to_list(v_vector)
                }})
        return objects_data

    def _serialize_lights(self, blender_scene: Scene) -> list:
        lights_data = []
        for light_object in blender_scene.objects:
            if light_object.type != 'LIGHT' or light_object.data.type != 'AREA':
                continue
            final_transform = CONVERSION_MATRIX_BLENDER_TO_PATHTRACER @ light_object.matrix_world
            blender_light_data = light_object.data
            if blender_light_data.shape == 'SQUARE':
                width = blender_light_data.size; height = blender_light_data.size
            else:
                width = blender_light_data.size; height = blender_light_data.size_y
            center_local = mu.Vector((0, 0, 0))
            u_edge_local = mu.Vector((width * 0.5, 0, 0))
            v_edge_local = mu.Vector((0, height * 0.5, 0))
            center_pathtracer = final_transform @ center_local
            u_edge_pathtracer = final_transform @ u_edge_local
            v_edge_pathtracer = final_transform @ v_edge_local
            u_vector = u_edge_pathtracer - center_pathtracer
            v_vector = v_edge_pathtracer - center_pathtracer
            u_vector.negate()
            lights_data.append({
                "pos": vector_to_list(center_pathtracer), "u": vector_to_list(u_vector),
                "v": vector_to_list(v_vector), "intensity": [blender_light_data.energy] * 3
            })
        if not lights_data:
            lights_data.append({ "pos": [0, 5, 0], "u": [2, 0, 0], "v": [0, 0, 2], "intensity": [25, 25, 25] })
        return lights_data

    def _serialize_camera(self, camera_object: bpy.types.Object, aperture: float) -> dict:
        blender_world_matrix = camera_object.matrix_world
        pos_blender = blender_world_matrix.translation
        forward_blender = (blender_world_matrix.to_3x3() @ mu.Vector((0, 0, -1))).normalized()
        up_blender = (blender_world_matrix.to_3x3() @ mu.Vector((0, 1, 0))).normalized()
        pos_pathtracer = CONVERSION_MATRIX_BLENDER_TO_PATHTRACER @ pos_blender
        look_at_pathtracer = CONVERSION_MATRIX_BLENDER_TO_PATHTRACER @ (pos_blender + forward_blender)
        up_pathtracer = (CONVERSION_MATRIX_BLENDER_TO_PATHTRACER.to_3x3() @ up_blender).normalized()
        return {
            "pos": vector_to_list(pos_pathtracer), "look_at": vector_to_list(look_at_pathtracer),
            "up": vector_to_list(up_pathtracer), "fov": math.degrees(camera_object.data.angle),
            "aperture": aperture
        }

    def _serialize_render_settings(self, blender_scene: Scene) -> dict:
        return {
            "width": blender_scene.render.resolution_x, "height": blender_scene.render.resolution_y,
            "samples": blender_scene.rs_props.samples
        }


# ───────────────────────── SCENE IMPORT OPERATOR ──────────────────────────────────

class RS_OT_ImportScene(Operator):
    bl_idname = "rs.import_scene"; bl_label = "Import from scene.json"
    filepath: StringProperty(subtype='FILE_PATH', default="scene.json")

    def execute(self, context: Context) -> set:
        filepath = Path(self.filepath)
        if not filepath.is_file():
            self.report({'ERROR'}, f"File not found: {self.filepath}"); return {'CANCELLED'}
        with filepath.open('r') as json_file:
            scene_data = json.load(json_file)
        self._clear_scene(context)
        materials_map = self._create_materials(scene_data.get("materials", {}))
        self._create_objects(context, scene_data.get("objects", []), materials_map)
        self._create_lights(context, scene_data.get("lights", []))
        self._setup_camera(context, scene_data.get("camera", {}))
        self._setup_render_settings(context, scene_data.get("render", {}), scene_data.get("camera", {}))
        self.report({'INFO'}, "Scene imported successfully."); return {'FINISHED'}

    def invoke(self, context: Context, event):
        context.window_manager.fileselect_add(self); return {'RUNNING_MODAL'}

    def _clear_scene(self, context: Context):
        objects_to_remove = [o for o in bpy.data.objects if PATHTRACER_OBJECT_ID_KEY in o or "RS_" in o.name]
        for obj in objects_to_remove: bpy.data.objects.remove(obj, do_unlink=True)
        mats_to_remove = [m for m in bpy.data.materials if hasattr(m, 'rs_props')]
        for mat in mats_to_remove:
            if not mat.users: bpy.data.materials.remove(mat)

    def _create_materials(self, materials_json: dict) -> dict:
        blender_mats = {}
        for name, props in materials_json.items():
            mat = bpy.data.materials.new(name); mat.use_nodes = False
            mat.diffuse_color = (*props.get("rgb", [1,0,1]), 1.0)
            mat.rs_props.metallic = props.get("metallic", 0.0); mat.rs_props.roughness = props.get("roughness", 0.5)
            mat.rs_props.ior = props.get("ior", 1.5); mat.rs_props.volume_density = props.get("volume_density", 0.0)
            mat.rs_props.volume_anisotropy = props.get("volume_anisotropy", 0.0); blender_mats[name] = mat
        return blender_mats

    def _create_objects(self, context: Context, objects_json: list, materials_map: dict):
        """Creates Blender objects from JSON by reconstructing their world matrix."""
        CONV_inv = CONVERSION_MATRIX_BLENDER_TO_PATHTRACER.inverted()
        for entry in objects_json:
            obj = None; desc = None
            if "sphere" in entry:
                desc = entry["sphere"]
                center_blender = CONV_inv @ mu.Vector(desc.get("center"))
                bpy.ops.mesh.primitive_uv_sphere_add(radius=desc.get("radius",1), location=center_blender)
                obj = context.active_object; obj[PATHTRACER_OBJECT_ID_KEY] = "sphere"
            elif "plane" in entry:
                desc = entry["plane"]
                center_pt = mu.Vector(desc.get("point")); u_pt = mu.Vector(desc.get("u")); v_pt = mu.Vector(desc.get("v"))
                normal_pt = u_pt.cross(v_pt)
                pathtracer_matrix = mu.Matrix(((u_pt.x, v_pt.x, normal_pt.x, center_pt.x),
                                               (u_pt.y, v_pt.y, normal_pt.y, center_pt.y),
                                               (u_pt.z, v_pt.z, normal_pt.z, center_pt.z),
                                               (0,      0,      0,           1          )))
                blender_matrix = CONV_inv @ pathtracer_matrix
                bpy.ops.mesh.primitive_plane_add(size=2.0)
                obj = context.active_object
                obj.matrix_world = blender_matrix
                obj[PATHTRACER_OBJECT_ID_KEY] = "plane"
            if obj and desc:
                obj.name = desc.get("name", "ImportedObject")
                if desc.get("mat") in materials_map: obj.data.materials.append(materials_map[desc.get("mat")])

    def _create_lights(self, context: Context, lights_json: list):
        """Creates Blender AREA lights by reconstructing their world matrix."""
        CONV_inv = CONVERSION_MATRIX_BLENDER_TO_PATHTRACER.inverted()
        for light_data in lights_json:
            center_pt = mu.Vector(light_data.get("pos")); u_pt = -mu.Vector(light_data.get("u")); v_pt = mu.Vector(light_data.get("v"))
            normal_pt = u_pt.cross(v_pt)
            pathtracer_matrix = mu.Matrix(((u_pt.x, v_pt.x, normal_pt.x, center_pt.x),
                                           (u_pt.y, v_pt.y, normal_pt.y, center_pt.y),
                                           (u_pt.z, v_pt.z, normal_pt.z, center_pt.z),
                                           (0,      0,      0,           1          )))
            blender_matrix = CONV_inv @ pathtracer_matrix
            bpy.ops.object.light_add(type='AREA')
            obj, light = context.active_object, context.active_object.data
            obj.name = "RS_ImportedLight"
            location, rotation, scale = blender_matrix.decompose()
            obj.location = location
            obj.rotation_euler = rotation.to_euler()
            obj.scale = (1, 1, 1)
            light.shape = 'RECTANGLE'
            light.size = scale.x * 2.0
            light.size_y = scale.y * 2.0
            light.energy = light_data.get("intensity", [25])[0]

    def _setup_camera(self, context: Context, camera_json: dict):
        CONV_inv = CONVERSION_MATRIX_BLENDER_TO_PATHTRACER.inverted()
        cam = context.scene.camera;
        if not cam:
            cam_data = bpy.data.cameras.new("RS_Camera"); cam = bpy.data.objects.new("RS_Camera", cam_data)
            context.scene.collection.objects.link(cam); context.scene.camera = cam
        pos = CONV_inv @ mu.Vector(camera_json.get("pos")); look_at = CONV_inv @ mu.Vector(camera_json.get("look_at"))
        up = (CONV_inv.to_3x3() @ mu.Vector(camera_json.get("up"))).normalized(); cam.location = pos
        cam.rotation_euler = create_look_at_quaternion(look_at - pos, up).to_euler()
        cam.data.angle = math.radians(camera_json.get("fov", 50.0))

    def _setup_render_settings(self, context: Context, render_json: dict, camera_json: dict):
        render = context.scene.render; render.resolution_x = render_json.get("width", 1280)
        render.resolution_y = render_json.get("height", 720); context.scene.rs_props.samples = render_json.get("samples", 128)
        context.scene.rs_props.aperture = camera_json.get("aperture", 0.01)

# ───────────────────────── UI PANEL & REGISTRATION ───────────────────────────────────────

class RS_PT_MainPanel(Panel):
    bl_label = "Rust Pathtracer"; bl_space_type = 'VIEW_3D'; bl_region_type = 'UI'; bl_category = 'Ray Scene'
    def draw(self, context: Context):
        layout = self.layout; obj = context.active_object
        box = layout.box(); col = box.column(align=True); col.label(text="Add Primitives", icon='ADD')
        row = col.row(align=True)
        row.operator(RS_OT_AddPlane.bl_idname, text="Plane", icon='MESH_PLANE')
        row.operator(RS_OT_AddSphere.bl_idname, text="Sphere", icon='MESH_UVSPHERE')
        col.operator(RS_OT_AddLight.bl_idname, text="Area Light", icon='LIGHT_AREA')
        if obj and PATHTRACER_OBJECT_ID_KEY in obj:
            box = layout.box(); col = box.column()
            if obj.active_material:
                mat = obj.active_material; col.label(text=f"Material: {mat.name}", icon='MATERIAL')
                col.prop(mat, "diffuse_color", text="Base Color")
                props = mat.rs_props
                for prop_name in props.bl_rna.properties.keys():
                    if prop_name not in ('rna_type', 'name'): col.prop(props, prop_name)
            else: col.label(text="Assign a material to see properties", icon='ERROR')
        box = layout.box(); col = box.column(align=True); col.label(text="Render Settings", icon='SCENE_DATA')
        col.prop(context.scene.rs_props, "samples"); col.prop(context.scene.rs_props, "aperture")
        layout.separator(); row = layout.row(align=True); row.scale_y = 1.5
        row.operator(RS_OT_ImportScene.bl_idname, text="Import Scene", icon='FILE_FOLDER')
        row.operator(RS_OT_ExportScene.bl_idname, text="Export Scene", icon='EXPORT')

classes = (
    PathtracerMaterialProperties, PathtracerSceneProperties,
    RS_OT_AddPlane, RS_OT_AddSphere, RS_OT_AddLight,
    RS_OT_ExportScene, RS_OT_ImportScene, RS_PT_MainPanel,
)
def register():
    for cls in classes: bpy.utils.register_class(cls)
    bpy.types.Material.rs_props = PointerProperty(type=PathtracerMaterialProperties)
    bpy.types.Scene.rs_props = PointerProperty(type=PathtracerSceneProperties)
def unregister():
    for cls in reversed(classes): bpy.utils.unregister_class(cls)
    del bpy.types.Material.rs_props; del bpy.types.Scene.rs_props
if __name__ == "__main__":
    register()