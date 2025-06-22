bl_info = {
    "name": "Rust-Ray Scene Builder",
    "author": "You",
    "description": "Build & import scenes for the Rust path-tracer",
    "version": (1, 2, 0),
    "blender": (4, 4, 3),
    "location": "3D-View > Sidebar > Ray Scene",
    "category": "Import-Export",
}

import bpy, json, math
import mathutils as mu
from pathlib import Path
from bpy.props import (
    FloatProperty, FloatVectorProperty, IntProperty, StringProperty,
    PointerProperty)
from bpy.types import Operator, Panel, PropertyGroup

# ───────────────────────── helpers
def to_list(v): return [v.x, v.y, v.z]
def vec(a):     return mu.Vector(a)

def look_at_quat(direction, up):
    """return quaternion that turns +Z into *direction*."""
    z = direction.normalized()
    x = up.cross(z).normalized()
    y = z.cross(x)
    m = mu.Matrix((x, y, z)).transposed()
    return m.to_quaternion()

# ───────────────────────── material props
class RS_MatProps(PropertyGroup):
    color:     FloatVectorProperty(name="RGB", subtype='COLOR', min=0, max=1, default=(1,1,1))
    metallic:  FloatProperty(name="Metallic",  min=0, max=2, default=0)
    roughness: FloatProperty(name="Roughness", min=0, max=1, default=1)
    ior:       FloatProperty(name="IOR",       min=1, max=3, default=1)

# ───────────────────────── scene-level render props
class RS_SceneProps(PropertyGroup):
    aperture: FloatProperty(name="Aperture", min=0, max=1, default=0.02, precision=3)
    samples:  IntProperty  (name="Samples",  min=1, max=4096, default=64)

# ───────────────────────── add primitives
class RS_OT_add_plane(Operator):
    bl_idname, bl_label = "rs.add_plane", "Add Plane"
    bl_options = {'REGISTER', 'UNDO'}
    def execute(self, ctx):
        bpy.ops.mesh.primitive_plane_add(size=2)
        ctx.active_object["rs_type"] = "plane"
        return {'FINISHED'}

class RS_OT_add_sphere(Operator):
    bl_idname, bl_label = "rs.add_sphere", "Add Sphere"
    bl_options = {'REGISTER', 'UNDO'}
    def execute(self, ctx):
        bpy.ops.mesh.primitive_uv_sphere_add(radius=1)
        ctx.active_object["rs_type"] = "sphere"
        return {'FINISHED'}

# ───────────────────────── EXPORT
class RS_OT_export(Operator):
    bl_idname, bl_label = "rs.export_scene", "Export scene.json"
    filepath: StringProperty(subtype="FILE_PATH", default="scene.json")

    def execute(self, ctx):
        scn = ctx.scene
        mats, objs = {}, []

        # collect objects & materials
        for ob in scn.objects:
            if "rs_type" not in ob:
                continue
            mp = ob.rs_mat
            mname = ob.name + "_Mat"
            mats[mname] = {
                "rgb": list(mp.color),
                "metallic": mp.metallic,
                "roughness": mp.roughness,
                "ior": mp.ior,
            }
            if ob["rs_type"] == "sphere":
                objs.append({"sphere":{
                    "center": to_list(ob.location),
                    "radius": ob.dimensions.x * 0.5,
                    "mat": mname}})
            else:
                n = ob.matrix_world.to_quaternion() @ mu.Vector((0,0,1))
                objs.append({"plane":{
                    "point": to_list(ob.location),
                    "normal": to_list(n),
                    "mat": mname}})

        # simple fixed light
        light = {"pos":[0,2.95,4], "u":[1,0,0], "v":[0,0,1], "intensity":[25,25,25]}

        # camera
        cam_ob = scn.camera
        if not cam_ob:
            self.report({'ERROR'}, "No active camera")
            return {'CANCELLED'}
        cam_dir = cam_ob.matrix_world @ mu.Vector((0,0,-1))
        cam_json = {
            "pos": to_list(cam_ob.location),
            "look_at": to_list(cam_dir),
            "up": to_list(cam_ob.matrix_world.to_quaternion() @ mu.Vector((0,1,0))),
            "fov": cam_ob.data.angle*180/math.pi,
            "aperture": scn.rs_props.aperture,
        }

        render_json = {
            "width": scn.render.resolution_x,
            "height": scn.render.resolution_y,
            "samples": scn.rs_props.samples,
        }

        scene = {"camera":cam_json, "render":render_json,
                 "materials":mats, "objects":objs, "light":light}

        with open(self.filepath, "w") as f:
            json.dump(scene, f, indent=2)
        self.report({'INFO'}, "Exported → "+self.filepath)
        return {'FINISHED'}

    def invoke(self, ctx, _): ctx.window_manager.fileselect_add(self); return {'RUNNING_MODAL'}

# ───────────────────────── IMPORT
class RS_OT_import(Operator):
    bl_idname, bl_label = "rs.import_scene", "Import scene.json"
    filepath: StringProperty(subtype="FILE_PATH", default="scene.json")

    def execute(self, ctx):
        data = json.load(open(self.filepath))
        scn = ctx.scene

        # set render
        rnd = data.get("render", {})
        scn.render.resolution_x = rnd.get("width", 800)
        scn.render.resolution_y = rnd.get("height", 600)
        scn.rs_props.samples    = rnd.get("samples", 64)
        scn.rs_props.aperture   = data.get("camera",{}).get("aperture",0.02)

        # clear old rs objects
        for ob in [o for o in scn.objects if "rs_type" in o]:
            bpy.data.objects.remove(ob, do_unlink=True)

        # materials dict
        mats_json = data.get("materials", {})
        mat_objs = {}
        for name, mj in mats_json.items():
            m = bpy.data.materials.new(name)
            m.use_nodes = False
            mat_objs[name] = m

        # make objects
        for obj in data.get("objects", []):
            if "sphere" in obj:
                s = obj["sphere"]
                bpy.ops.mesh.primitive_uv_sphere_add(radius=s["radius"])
                ob = ctx.active_object
                ob.location = vec(s["center"])
                ob["rs_type"] = "sphere"
                ob.rs_mat.color = s_mat = mats_json[s["mat"]]["rgb"]
            else:  # plane
                p = obj["plane"]
                bpy.ops.mesh.primitive_plane_add(size=2)
                ob = ctx.active_object
                ob.location = vec(p["point"])
                normal = vec(p["normal"])
                quat = look_at_quat(normal, mu.Vector((0,1,0)))
                ob.rotation_euler = quat.to_euler()
                ob["rs_type"] = "plane"

            # copy material props
            mj = mats_json[obj.get("sphere",obj.get("plane"))["mat"]]
            ob.rs_mat.color     = mj["rgb"]
            ob.rs_mat.metallic  = mj["metallic"]
            ob.rs_mat.roughness = mj["roughness"]
            ob.rs_mat.ior       = mj["ior"]

        # camera
        cam_json = data.get("camera",{})
        cam_ob = scn.camera or bpy.data.objects.new("RayCamera", bpy.data.cameras.new("RayCamera"))
        if cam_ob.name not in scn.objects:
            scn.collection.objects.link(cam_ob)
            scn.camera = cam_ob
        cam_ob.location = vec(cam_json.get("pos", [0, 0, 0]))
        look = vec(cam_json.get("look_at", [0, 0, 1]))
        up   = vec(cam_json.get("up", [0, 1, 0]))

        # look_at is exported as a point one unit in front of the camera
        # along its viewing direction. When computing the orientation we need
        # the vector pointing from that target back to the camera so that the
        # resulting quaternion matches the original camera rotation.
        cam_ob.rotation_euler = look_at_quat(cam_ob.location - look, up).to_euler()
        cam_ob.data.angle = math.radians(cam_json.get("fov",60))

        self.report({'INFO'},"Scene imported")
        return {'FINISHED'}

    def invoke(self, ctx, _): ctx.window_manager.fileselect_add(self); return {'RUNNING_MODAL'}

# ───────────────────────── UI
class RS_PT_panel(Panel):
    bl_label, bl_space_type = "Ray Scene", 'VIEW_3D'
    bl_region_type, bl_category = 'UI', 'Ray Scene'

    def draw(self, ctx):
        l = self.layout
        l.operator("rs.add_plane")
        l.operator("rs.add_sphere")
        l.operator("rs.import_scene", icon='FILE_FOLDER')
        l.separator()
        ob = ctx.object
        if ob and hasattr(ob, "rs_mat"):
            box=l.box(); box.label(text="Material")
            box.prop(ob.rs_mat,"color"); box.prop(ob.rs_mat,"metallic")
            box.prop(ob.rs_mat,"roughness"); box.prop(ob.rs_mat,"ior")
        l.separator()
        box=l.box(); box.label(text="Render Settings")
        box.prop(ctx.scene.rs_props,"aperture")
        box.prop(ctx.scene.rs_props,"samples")
        l.separator()
        l.operator("rs.export_scene", icon='EXPORT')

# ───────────────────────── registration
CLASSES = (
    RS_MatProps, RS_SceneProps,
    RS_OT_add_plane, RS_OT_add_sphere,
    RS_OT_export, RS_OT_import, RS_PT_panel)

def register():
    for c in CLASSES: bpy.utils.register_class(c)
    bpy.types.Object.rs_mat = PointerProperty(type=RS_MatProps)
    bpy.types.Scene.rs_props = PointerProperty(type=RS_SceneProps)

def unregister():
    for c in reversed(CASSES): bpy.utils.unregister_class(c)
    del bpy.types.Object.rs_mat, bpy.types.Scene.rs_props

if __name__ == "__main__":
    register()
