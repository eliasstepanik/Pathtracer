bl_info = {
    "name"       : "Rust-Ray Scene Builder",
    "author"     : "Elias Stepanik",
    "description": "Build / import scenes for the Rust path-tracer",
    "version"    : (1, 4, 0),
    "blender"    : (4, 4, 3),
    "location"   : "3D-View ▸ Sidebar ▸ Ray Scene",
    "category"   : "Import-Export",
}

import bpy, json, math, mathutils as mu
from pathlib import Path
from bpy.props import FloatProperty, FloatVectorProperty, IntProperty, StringProperty, PointerProperty, EnumProperty
from bpy.types  import Operator, Panel, PropertyGroup

# ───────────────────────── helpers ─────────────────────────
def to_list(v: mu.Vector) -> list[float]:
    return [v.x, v.z, -v.y]

def from_list(a: list[float]) -> mu.Vector:
    return mu.Vector((a[0], -a[2], a[1]))

EPS = 1e-6
WORLD_X = mu.Vector((1, 0, 0))
WORLD_Y = mu.Vector((0, 1, 0))
def look_at_quat(direction: mu.Vector, up: mu.Vector = WORLD_Y) -> mu.Quaternion:
    z = direction.normalized()
    if abs(z.dot(up)) > 1.0 - EPS:
        up = WORLD_X if abs(z.dot(WORLD_X)) < 1.0 - EPS else WORLD_Y
    x = up.cross(z).normalized()
    y = z.cross(x)
    return mu.Matrix((x, y, z)).transposed().to_quaternion()

# ───────────────────────── material props ──────────────────
# MODIFIED: These properties are now on bpy.types.Material
class RS_MatProps(PropertyGroup):
    metallic  : FloatProperty(name="Metallic" , min=0, max=2, default=0)
    roughness : FloatProperty(name="Roughness", min=0, max=1, default=1)
    ior       : FloatProperty(name="IOR"      , min=0, max=3, default=1.0) # Default to 1.0 (air)

# ───────────────────────── scene-level props ───────────────
class RS_SceneProps(PropertyGroup):
    aperture : FloatProperty(name="Aperture", min=0, max=1, default=0.02, precision=3)
    samples  : IntProperty  (name="Samples" , min=1, max=65536, default=64)

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
        mats, objs, lights, used_mats = {}, [], [], set()

        # --- MODIFIED: Gather used materials first ---
        for ob in scn.objects:
            if "rs_type" in ob and ob.active_material:
                used_mats.add(ob.active_material)

        for mat in used_mats:
            mats[mat.name] = {
                "rgb": list(mat.diffuse_color)[:3],
                "metallic": mat.rs_props.metallic,
                "roughness": mat.rs_props.roughness,
                "ior": mat.rs_props.ior
            }

        # ――― objects
        for ob in scn.objects:
            if "rs_type" not in ob or not ob.active_material: continue

            base_obj = {
                "name": ob.name,
                "mat": ob.active_material.name
            }

            if ob["rs_type"] == "sphere":
                objs.append({"sphere":{
                    **base_obj,
                    "center": to_list(ob.location),
                    "radius": ob.dimensions.x * 0.5
                }})
            else: # plane - ENTIRELY NEW LOGIC HERE
                # Get the object's world matrix and dimensions
                mat_world = ob.matrix_world
                dims = ob.dimensions

                # Create vectors for half-width and half-height in the object's local space
                u_local = mu.Vector((dims.x * 0.5, 0, 0))
                v_local = mu.Vector((0, dims.y * 0.5, 0))

                # Transform these vectors into world space using the rotation part of the matrix
                u_world = mat_world.to_3x3() @ u_local
                v_world = mat_world.to_3x3() @ v_local

                objs.append({"plane":{
                    **base_obj,
                    "point" : to_list(ob.location),
                    "u"     : to_list(u_world),
                    "v"     : to_list(v_world)
                }})

        # ――― lights (unchanged)
        for ob in scn.objects:
            if ob.type!='LIGHT' or ob.data.type!='AREA': continue
            sz_x, sz_y = getattr(ob.data,'size',2.0), getattr(ob.data,'size_y',2.0)
            q = ob.matrix_world.to_quaternion()
            u, v = (q @ mu.Vector((1,0,0))).normalized()*sz_x, (q @ mu.Vector((0,1,0))).normalized()*sz_y
            lights.append({"pos":to_list(ob.location), "u":to_list(u), "v":to_list(v), "intensity":[ob.data.energy]*3})
        if not lights:
            lights.append({"pos":[0,2.95,4], "u":[1,0,0], "v":[0,0,1], "intensity":[25,25,25]})

        # ――― camera (unchanged)
        cam = scn.camera
        if cam is None: self.report({'ERROR'},"No active camera"); return {'CANCELLED'}
        forward = cam.matrix_world.to_3x3() @ mu.Vector((0, 0, -1))
        cam_json = {"pos":to_list(cam.location), "look_at":to_list(cam.location + forward),
                    "up":to_list(cam.matrix_world.to_3x3() @ mu.Vector((0, 1, 0))),
                    "fov":cam.data.angle*180/math.pi, "aperture":scn.rs_props.aperture}

        scene = { "camera":cam_json, "render":{"width":scn.render.resolution_x, "height":scn.render.resolution_y, "samples":scn.rs_props.samples},
                  "materials":mats, "objects":objs, "lights":lights }

        with open(self.filepath,"w") as f: json.dump(scene,f,indent=2)
        self.report({'INFO'},"Exported → "+self.filepath); return {'FINISHED'}

    def invoke(self,ctx,_): ctx.window_manager.fileselect_add(self); return {'RUNNING_MODAL'}

# ───────────────────────── IMPORT ──────────────────────────
class RS_OT_import(Operator):
    bl_idname = "rs.import_scene"; bl_label = "Import scene.json"
    filepath : StringProperty(subtype='FILE_PATH', default="scene.json")

    def execute(self, ctx):
        path = Path(self.filepath)
        if not path.is_file(): self.report({'ERROR'},"File not found"); return {'CANCELLED'}
        data, scn = json.load(path.open()), ctx.scene

        # --- render settings ---
        rnd, cam_json = data.get("render",{}), data.get("camera",{})
        scn.render.resolution_x, scn.render.resolution_y = rnd.get("width",800), rnd.get("height",600)
        scn.rs_props.samples, scn.rs_props.aperture = rnd.get("samples",64), cam_json.get("aperture",0.02)

        for ob in [o for o in scn.objects if ("rs_type" in o) or (o.type=='LIGHT' and o.data.type=='AREA')]:
            bpy.data.objects.remove(ob,do_unlink=True)
        for mat in [m for m in bpy.data.materials if m.name in data.get("materials",{})]:
            bpy.data.materials.remove(mat)

        # --- MODIFIED: Create materials from the library ---
        mats_json = data.get("materials",{})
        for name, mj in mats_json.items():
            mat = bpy.data.materials.new(name)
            mat.use_nodes = False
            mat.diffuse_color = (*mj["rgb"], 1.0) # Set standard color
            mat.rs_props.metallic = mj["metallic"]
            mat.rs_props.roughness = mj["roughness"]
            mat.rs_props.ior = mj["ior"]

        # --- MODIFIED: Create objects and assign materials by name ---
        for entry in data.get("objects",[]):
            desc = entry.get("sphere") or entry.get("plane")
            if "sphere" in entry:
                bpy.ops.mesh.primitive_uv_sphere_add(radius=desc["radius"])
                ob = ctx.active_object; ob.location = from_list(desc["center"])
                ob["rs_type"] = "sphere"
            else: # plane
                bpy.ops.mesh.primitive_plane_add(size=2)
                ob = ctx.active_object; ob.location = from_list(desc["point"])
                ob.rotation_euler = look_at_quat(from_list(desc["normal"])).to_euler()
                ob.scale.x, ob.scale.y = desc.get("size",[2,2])[0]*0.5, desc.get("size",[2,2])[1]*0.5
                ob["rs_type"] = "plane"

            ob.name = desc["name"] # ADDED: Set object name
            if desc["mat"] in bpy.data.materials:
                ob.active_material = bpy.data.materials[desc["mat"]]

        # --- lights --- (unchanged)
        for lj in data.get("lights",[]):
            data_l = bpy.data.lights.new("RS_Light","AREA"); data_l.shape = 'RECTANGLE'
            data_l.energy, u, v = lj["intensity"][0], from_list(lj["u"]), from_list(lj["v"])
            data_l.size, data_l.size_y = u.length, v.length
            lob = bpy.data.objects.new("RS_Light", data_l); scn.collection.objects.link(lob)
            lob.location = from_list(lj["pos"])
            lob.rotation_euler = mu.Matrix((u.normalized(),v.normalized(),(u.cross(v)).normalized())).to_quaternion().to_euler()

        # --- camera --- (unchanged)
        cam = scn.camera or bpy.data.objects.new("RayCam", bpy.data.cameras.new("RayCam"))
        if cam.name not in scn.objects: scn.collection.objects.link(cam); scn.camera = cam
        cam.location = from_list(cam_json.get("pos",[0,0,0]))
        look = from_list(cam_json.get("look_at",[0,0,1])) - cam.location
        cam.rotation_euler = look_at_quat(look, from_list(cam_json.get("up",[0,1,0]))).to_euler()
        cam.data.angle = math.radians(cam_json.get("fov",60))

        self.report({'INFO'},"Scene imported"); return {'FINISHED'}

    def invoke(self,ctx,_): ctx.window_manager.fileselect_add(self); return {'RUNNING_MODAL'}

# ───────────────────────── UI panel ────────────────────────
class RS_PT_panel(Panel):
    bl_label="Ray Scene"; bl_space_type='VIEW_3D'; bl_region_type='UI'; bl_category='Ray Scene'

    def draw(self, ctx):
        l = self.layout
        l.operator("rs.add_plane"); l.operator("rs.add_sphere")
        l.operator("rs.add_light", icon='LIGHT_AREA')
        l.operator("rs.import_scene", icon='FILE_FOLDER')
        l.separator()

        ob = ctx.object
        # --- MODIFIED: Show properties of the object's active material ---
        if ob and "rs_type" in ob and ob.active_material:
            mat = ob.active_material
            box = l.box(); box.label(text=f"Material: {mat.name}")
            # The color is now Blender's standard material color
            box.prop(mat, "diffuse_color", text="Color")
            # Custom properties are on the material's 'rs_props'
            for p in ("metallic","roughness","ior"):
                box.prop(mat.rs_props, p)
        elif ob and "rs_type" in ob:
            l.label(text="Assign a material to see properties.", icon='ERROR')

        l.separator()
        box = l.box(); box.label(text="Render Settings")
        box.prop(ctx.scene.rs_props, "aperture"); box.prop(ctx.scene.rs_props, "samples")
        l.separator()
        l.operator("rs.export_scene", icon='EXPORT')

# ───────────────────────── registration ────────────────────
classes = ( RS_MatProps, RS_SceneProps, RS_OT_add_plane, RS_OT_add_sphere, RS_OT_add_light,
            RS_OT_export, RS_OT_import, RS_PT_panel )

def register():
    for c in classes: bpy.utils.register_class(c)
    # MODIFIED: Custom properties are now on Material and Scene
    bpy.types.Material.rs_props = PointerProperty(type=RS_MatProps)
    bpy.types.Scene.rs_props    = PointerProperty(type=RS_SceneProps)

def unregister():
    for c in reversed(classes): bpy.utils.unregister_class(c)
    del bpy.types.Material.rs_props
    del bpy.types.Scene.rs_props

if __name__ == "__main__": register()