"""Builds the RustCraft block models from scratch.

Run from the repository root:

    blender --background --python assets/models/build.py

Writes blocks.blend (one scene per model) and one .glb per model next to this file, then runs
embed.py, which packs the .glb files into docs/showcase/js/glb.js for the guide page.

A model fills one block: 1 x 1 x 1 Blender units, Z up, origin in the corner of the block. Boxes
are given in texture pixels, 16 to a block. The models themselves live in the section files
(transport.py, machines.py, energy.py, storage.py, gear.py,
branches.py); kit.py holds the shared palette and parts.
Every part of a model is one Blender object, named as the part, with its origin at the part's
pivot. The naming rules the guide reads are listed in README.md.
"""
import glob
import math
import os
import runpy
import sys

import bmesh
import bpy
from mathutils import Matrix

HERE = os.path.dirname(os.path.abspath(__file__))
# The section files are imported from here; no __pycache__ is left in the repository.
sys.dont_write_bytecode = True
sys.path.insert(0, HERE)

import branches  # noqa: E402
import energy  # noqa: E402
import gear  # noqa: E402
import kit  # noqa: E402
import machines  # noqa: E402
import storage  # noqa: E402
import transport  # noqa: E402

PX = 1 / 16
CENTRE = (8, 8, 0)
SECTIONS = (transport, machines, energy, storage, gear, branches)


def linear(color):
    """Blender stores material colours in linear light; the palette is written in sRGB."""
    def channel(c):
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4
    return [channel(int(color[i:i + 2], 16) / 255) for i in (1, 3, 5)] + [1.0]


def palette():
    """The shared colours plus the colours of every section; a name means one colour everywhere."""
    colors = dict(kit.COLORS)
    for section in SECTIONS:
        for name, color in section.COLORS.items():
            assert colors.setdefault(name, color) == color, 'colour %s differs between sections' % name
    return colors


COLORS = palette()


class Model:
    """Collects boxes by part; every part becomes one Blender object."""

    def __init__(self):
        self.parts = {}
        self.pivots = {}

    def pivot(self, part, x, y, z):
        """Puts the origin of a part at (x, y, z) pixels: the point the guide turns it about."""
        self.pivots[part] = (x, y, z)

    def box(self, mat, x, y, z, w, d, h, part='body', tilt=None, pivot=None, ring=False, quarter=0):
        """Adds a box, in pixels. tilt = (axis, degrees) turns it about `pivot`, by default the
        centre of its bottom face. ring=True repeats it on all four sides of the block; quarter=n
        turns it n quarter turns about the vertical axis of the block."""
        mesh, materials = self.parts.setdefault(part, (bmesh.new(), []))
        if mat not in materials:
            materials.append(mat)
        own = Matrix.Translation((x + w / 2, y + d / 2, z + h / 2)) @ Matrix.Diagonal((w, d, h, 1))
        if tilt:
            at = Matrix.Translation(pivot or (x + w / 2, y + d / 2, z))
            own = at @ Matrix.Rotation(math.radians(tilt[1]), 4, tilt[0]) @ at.inverted() @ own
        around = Matrix.Translation(CENTRE)
        for turn in range(quarter, quarter + (4 if ring else 1)):
            side = around @ Matrix.Rotation(turn * math.pi / 2, 4, 'Z') @ around.inverted()
            made = bmesh.ops.create_cube(mesh, size=1, matrix=Matrix.Scale(PX, 4) @ side @ own)
            for face in {f for v in made['verts'] for f in v.link_faces}:
                face.material_index = materials.index(mat)


def material(name):
    mat = bpy.data.materials.get(name)
    if mat:
        return mat
    mat = bpy.data.materials.new(name)
    color = linear(COLORS[name])
    mat.diffuse_color = color
    if not mat.node_tree:
        mat.use_nodes = True
    bsdf = next(n for n in mat.node_tree.nodes if n.type == 'BSDF_PRINCIPLED')
    bsdf.inputs['Base Color'].default_value = color
    bsdf.inputs['Roughness'].default_value = 1.0
    return mat


def build(name, draw):
    scene = bpy.data.scenes.new(name)
    model = Model()
    draw(model)
    for part, (mesh, materials) in model.parts.items():
        origin = [c * PX for c in model.pivots.get(part, (0, 0, 0))]
        bmesh.ops.translate(mesh, vec=[-c for c in origin], verts=mesh.verts)
        data = bpy.data.meshes.new(name + '_' + part)
        mesh.to_mesh(data)
        mesh.free()
        for mat in materials:
            data.materials.append(material(mat))
        # Blender keeps object names unique by adding .001, .002; the guide drops that suffix.
        obj = bpy.data.objects.new(part, data)
        obj.location = origin
        scene.collection.objects.link(obj)
    return scene


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    default = bpy.data.scenes[0]
    models = {}
    for section in SECTIONS:
        models.update(section.MODELS)
    scenes = [build(name, draw) for name, draw in models.items()]
    bpy.data.scenes.remove(default)
    # Models dropped from MODELS must not stay in the guide: embed.py packs every .glb it finds.
    for old in glob.glob(os.path.join(HERE, '*.glb')):
        os.remove(old)
    for scene in scenes:
        bpy.context.window.scene = scene
        bpy.ops.export_scene.gltf(
            filepath=os.path.join(HERE, scene.name + '.glb'), export_format='GLB', use_active_scene=True,
            export_apply=True, export_yup=True, export_normals=False, export_materials='EXPORT',
            export_texcoords=False, export_animations=False, export_skins=False, export_morph=False,
            export_cameras=False, export_lights=False, export_extras=False)
    bpy.context.window.scene = scenes[0]
    bpy.ops.wm.save_as_mainfile(filepath=os.path.join(HERE, 'blocks.blend'), compress=True)
    runpy.run_path(os.path.join(HERE, 'embed.py'), run_name='__main__')


main()
