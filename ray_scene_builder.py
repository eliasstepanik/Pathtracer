bl_info = {
    "name"       : "Rust-Ray Scene Builder",
    "author"     : "Elias Stepanik",
    "description": "Build / import scenes for the Rust path-tracer",
    "version"    : (1, 3, 3),
    "blender"    : (4, 4, 3),
    "location"   : "3D-View ▸ Sidebar ▸ Ray Scene",
    "category"   : "Import-Export",
}

import bpy, json, math, mathutils as mu
from pathlib import Path
from bpy.props import FloatProperty, FloatVectorProperty, IntProperty, StringProperty, PointerProperty
from bpy.types  import Operator, Panel, PropertyGroup

# ───────────────────────── helpers ─────────────────────────
def to_list(v: mu.Vector) -> list[float]:
    """Rotate Blender's Z-up vector into the tracer's Y-up space."""
    # Rotate around X so Blender Z (up) becomes tracer Y (up) while
    # preserving a right handed basis. Simply swapping Y and Z would flip
    # the coordinate system handedness which leads to inverted normals.
    return [v.x, v.z, -v.y]

def from_list(a: list[float]) -> mu.Vector:
    """Inverse of :func:`to_list` for converting back to Blender."""
    return mu.Vector((a[0], -a[2], a[1]))

def look_at_quat(direction: mu.Vector, up: mu.Vector) -> mu.Quaternion:
    """Return quaternion that points +Z (Blender forward) into `direction`."""
    z = direction.normalized()
    x = up.cross(z).normalized()
    y = z.cross(x)
    return mu.Matrix((x, y, z)).transposed().to_quaternion()

# ───────────────────────── material props ──────────────────
class RS_MatProps(PropertyGroup):
    color     : FloatVectorProperty(name="RGB"      , subtype='COLOR', min=0, max=1, default=(1,1,1))
    metallic  : FloatProperty      (name="Metallic" , min=0, max=2, default=0)
    roughness : FloatProperty      (name="Roughness", min=0, max=1, default=1)
    ior       : FloatProperty      (name="IOR"      , min=1, max=3, default=1)

# ───────────────────────── scene-level props ───────────────
class RS_SceneProps(PropertyGroup):
    aperture : FloatProperty(name="Aperture", min=0, max=1, default=0.02, precision=3)
    samples  : IntProperty  (name="Samples" , min=1, max=4096, default=64)

# ───────────────────────── add primitives ──────────────────
class RS_OT_add_plane(Operator):
    bl_idname = "rs.add_plane"; bl_label = "Add Plane"
    bl_options = {'REGISTER','UNDO'}
    def execute(self, ctx):
        bpy.ops.mesh.primitive_plane_add(size=2)
        ctx.active_object["rs_type"] = "plane"
        return {'FINISHED'}

class RS_OT_add_sphere(Operator):
    bl_idname = "rs.add_sphere"; bl_label = "Add Sphere"
    bl_options = {'REGISTER','UNDO'}
    def execute(self, ctx):
        bpy.ops.mesh.primitive_uv_sphere_add(radius=1)
        ctx.active_object["rs_type"] = "sphere"
        return {'FINISHED'}

class RS_OT_add_light(Operator):
    """Add rectangular area-light compatible with the tracer"""
    bl_idname = "rs.add_light"; bl_label = "Add Light"
    bl_options = {'REGISTER','UNDO'}
    size_x : FloatProperty(name="Width",  default=2, min=0.01)
    size_y : FloatProperty(name="Height", default=2, min=0.01)
    energy : FloatProperty(name="Intensity", default=25, min=0)
    def execute(self, ctx):
        data = bpy.data.lights.new("RS_Light","AREA")
        data.shape, data.size, data.size_y, data.energy = 'RECTANGLE', self.size_x, self.size_y, self.energy
        ob = bpy.data.objects.new("RS_Light", data)
        ctx.collection.objects.link(ob)
        return {'FINISHED'}

# ───────────────────────── EXPORT ──────────────────────────
class RS_OT_export(Operator):
    bl_idname = "rs.export_scene"; bl_label = "Export scene.json"
    filepath : StringProperty(subtype='FILE_PATH', default="scene.json")

    def execute(self, ctx):
        scn = ctx.scene
        mats, objs, lights = {}, [], []

        # ――― objects & materials
        for ob in scn.objects:
            if "rs_type" not in ob: continue
            mp, mname = ob.rs_mat, f"{ob.name}_Mat"
            mats[mname] = {
                "rgb": list(mp.color),
                "metallic": mp.metallic,
                "roughness": mp.roughness,
                "ior": mp.ior
            }

            if ob["rs_type"] == "sphere":
                objs.append({"sphere":{
                    "center": to_list(ob.location),
                    "radius": ob.dimensions.x * 0.5,
                    "mat"   : mname}})
            else:
                n   = ob.matrix_world.to_quaternion() @ mu.Vector((0,0,1))
                sz  = ob.dimensions  # world-space X/Y
                objs.append({"plane":{
                    "point" : to_list(ob.location),
                    "normal": to_list(n),
                    "size"  : [sz.x, sz.y],
                    "mat"   : mname}})

        # ――― lights
        for ob in scn.objects:
            if ob.type!='LIGHT' or ob.data.type!='AREA': continue
            sz_x = getattr(ob.data,'size',2.0)
            sz_y = getattr(ob.data,'size_y',sz_x)
            q    = ob.matrix_world.to_quaternion()
            u    = (q @ mu.Vector((1,0,0))).normalized()*sz_x
            v    = (q @ mu.Vector((0,1,0))).normalized()*sz_y
            lights.append({"pos":to_list(ob.location),
                           "u":to_list(u),"v":to_list(v),
                           "intensity":[ob.data.energy]*3})
        if not lights:  # fallback
            lights.append({
                "pos":[0,2.95,4],
                "u":[1,0,0],"v":[0,0,1],
                "intensity":[25,25,25]
            })

        # ――― camera
        cam = scn.camera
        if cam is None:
            self.report({'ERROR'},"No active camera"); return {'CANCELLED'}
        # Camera orientation: local -Z points along the viewing direction and
        # +Y is the camera's up axis in Blender. Convert both vectors so the
        # tracer receives a proper look-at and up description.
        forward = cam.matrix_world.to_3x3() @ mu.Vector((0, 0, -1))
        cam_json = {
            "pos"     : to_list(cam.location),
            "look_at" : to_list(cam.location + forward),
            "up"      : to_list(cam.matrix_world.to_3x3() @ mu.Vector((0, 1, 0))),
            "fov"     : cam.data.angle * 180 / math.pi,
            "aperture": scn.rs_props.aperture,
        }

        render_json = {
            "width":  scn.render.resolution_x,
            "height": scn.render.resolution_y,
            "samples":scn.rs_props.samples
        }

        scene = {
            "camera"   : cam_json,
            "render"   : render_json,
            "materials": mats,
            "objects"  : objs,
            "lights"   : lights
        }

        with open(self.filepath,"w") as f:
            json.dump(scene,f,indent=2)

        self.report({'INFO'},"Exported → "+self.filepath)
        return {'FINISHED'}

    def invoke(self,ctx,_):
        ctx.window_manager.fileselect_add(self)
        return {'RUNNING_MODAL'}

# ───────────────────────── IMPORT ──────────────────────────
class RS_OT_import(Operator):
    bl_idname = "rs.import_scene"; bl_label = "Import scene.json"
    filepath : StringProperty(subtype='FILE_PATH', default="scene.json")

    def execute(self, ctx):
        path = Path(self.filepath)
        if not path.is_file():
            self.report({'ERROR'},"File not found"); return {'CANCELLED'}
        data = json.load(path.open())
        scn  = ctx.scene

        # ――― render settings
        rnd = data.get("render",{})
        scn.render.resolution_x = rnd.get("width" ,800)
        scn.render.resolution_y = rnd.get("height",600)
        scn.rs_props.samples    = rnd.get("samples",64)
        scn.rs_props.aperture   = data.get("camera",{}).get("aperture",0.02)

        # ――― clear previous RS objects/lights
        for ob in [o for o in scn.objects if ("rs_type" in o) or (o.type=='LIGHT' and o.data.type=='AREA')]:
            bpy.data.objects.remove(ob,do_unlink=True)

        # ――― materials
        mats_json = data.get("materials",{})
        for name in mats_json:
            if name not in bpy.data.materials:
                bpy.data.materials.new(name)
            bpy.data.materials[name].use_nodes = False

        # ――― objects
        for entry in data.get("objects",[]):
            if "sphere" in entry:
                s = entry["sphere"]
                bpy.ops.mesh.primitive_uv_sphere_add(radius=s["radius"])
                ob = ctx.active_object
                ob.location = from_list(s["center"])
                ob["rs_type"] = "sphere"
            else:
                p = entry["plane"]
                bpy.ops.mesh.primitive_plane_add(size=2)
                ob = ctx.active_object
                ob.location = from_list(p["point"])
                ob["rs_type"] = "plane"
                # orient
                n = from_list(p["normal"])
                ob.rotation_euler = look_at_quat(n, mu.Vector((0,0,1))).to_euler()
                # scale to size (default plane is 2×2)
                size = p.get("size",[2,2])
                ob.scale.x, ob.scale.y = size[0]*0.5, size[1]*0.5

            mj = mats_json[(entry.get("sphere") or entry.get("plane"))["mat"]]
            ob.rs_mat.color     = mj["rgb"]
            ob.rs_mat.metallic  = mj["metallic"]
            ob.rs_mat.roughness = mj["roughness"]
            ob.rs_mat.ior       = mj["ior"]

        # ――― lights
        for lj in data.get("lights",[]):
            data_l = bpy.data.lights.new("RS_Light","AREA")
            data_l.shape = 'RECTANGLE'
            data_l.energy = lj["intensity"][0]
            u, v = from_list(lj["u"]), from_list(lj["v"])
            data_l.size   = u.length
            data_l.size_y = v.length
            lob = bpy.data.objects.new("RS_Light", data_l)
            scn.collection.objects.link(lob)
            lob.location = from_list(lj["pos"])
            quat = mu.Matrix((u.normalized(), v.normalized(), (u.cross(v)).normalized())).to_quaternion()
            lob.rotation_euler = quat.to_euler()

        # ――― camera
        cam_json = data.get("camera",{})
        cam = scn.camera or bpy.data.objects.new("RayCam", bpy.data.cameras.new("RayCam"))
        if cam.name not in scn.objects:
            scn.collection.objects.link(cam)
            scn.camera = cam
        cam.location = from_list(cam_json.get("pos",[0,0,0]))
        look = from_list(cam_json.get("look_at",[0,0,1])) - cam.location
        up   = from_list(cam_json.get("up",[0,1,0]))
        cam.rotation_euler = look_at_quat(look, up).to_euler()
        cam.data.angle     = math.radians(cam_json.get("fov",60))

        self.report({'INFO'},"Scene imported")
        return {'FINISHED'}

    def invoke(self,ctx,_):
        ctx.window_manager.fileselect_add(self)
        return {'RUNNING_MODAL'}

# ───────────────────────── UI panel ────────────────────────
class RS_PT_panel(Panel):
    bl_label       = "Ray Scene"
    bl_space_type  = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category    = 'Ray Scene'

    def draw(self, ctx):
        l = self.layout
        l.operator("rs.add_plane");  l.operator("rs.add_sphere")
        l.operator("rs.add_light", icon='LIGHT_AREA')
        l.operator("rs.import_scene", icon='FILE_FOLDER')
        l.separator()

        ob = ctx.object
        if ob and hasattr(ob,"rs_mat"):
            box = l.box(); box.label(text="Material")
            for p in ("color","metallic","roughness","ior"):
                box.prop(ob.rs_mat, p)
        l.separator()
        box = l.box(); box.label(text="Render Settings")
        box.prop(ctx.scene.rs_props, "aperture")
        box.prop(ctx.scene.rs_props, "samples")
        l.separator()
        l.operator("rs.export_scene", icon='EXPORT')

# ───────────────────────── registration ────────────────────
classes = (
    RS_MatProps, RS_SceneProps,
    RS_OT_add_plane, RS_OT_add_sphere, RS_OT_add_light,
    RS_OT_export, RS_OT_import, RS_PT_panel
)

def register():
    for c in classes:
        bpy.utils.register_class(c)
    bpy.types.Object.rs_mat   = PointerProperty(type=RS_MatProps)
    bpy.types.Scene.rs_props  = PointerProperty(type=RS_SceneProps)

def unregister():
    for c in reversed(classes):
        bpy.utils.unregister_class(c)
    del bpy.types.Object.rs_mat, bpy.types.Scene.rs_props

if __name__ == "__main__":
    register()
