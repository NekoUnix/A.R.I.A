"""Regenerate ARIA's original MIT-licensed starter art; Python 3, no packages required."""
import base64
import json
import math
from pathlib import Path
import struct
import wave
import zlib

ROOT = Path(__file__).resolve().parent
ASSETS = ROOT / "assets"
ASSETS.mkdir(exist_ok=True)

def text(path, value):
    path.write_text(value, encoding="utf-8", newline="\n")

def document(path, value):
    text(path, json.dumps(value, indent=2) + "\n")

def png(name, color, star=False):
    size = 128
    raw = bytearray()
    for y in range(size):
        raw.append(0)
        for x in range(size):
            dx, dy = (x + .5) / 64 - 1, (y + .5) / 64 - 1
            r = .50 + .30 * math.cos(5 * math.atan2(dy, dx) - math.pi / 2) if star else .68 * (1 + dy * .22)
            a = int(255 * max(0, min(1, (r - math.hypot(dx, dy)) * 64)))
            raw.extend((*color, a))
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">2I5B", size, size, 8, 6, 0, 0, 0))
    data += chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")
    (ASSETS / name).write_bytes(data)

png("star.png", (255, 212, 91), True)
png("droplet.png", (255, 255, 255))
text(ASSETS / "star.svg", '''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <!-- Edit the fill and points; export a transparent PNG for ARIA. -->
  <path fill="#FFD45B" d="M64 8 79 45 119 48 88 75 97 115 64 94 31 115 40 75 9 48 49 45Z"/>
</svg>
''')
text(ASSETS / "droplet.svg", '''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <!-- White artwork can be tinted to any liquid color inside ARIA. -->
  <path fill="#FFFFFF" d="M64 8C54 30 25 54 25 80a39 39 0 0 0 78 0C103 54 74 30 64 8Z"/>
</svg>
''')
corners = [(-.5,-.5,-.5),(.5,-.5,-.5),(.5,.5,-.5),(-.5,.5,-.5),(-.5,-.5,.5),(.5,-.5,.5),(.5,.5,.5),(-.5,.5,.5)]
faces = [(0,3,2,1),(4,5,6,7),(0,4,7,3),(1,2,6,5),(3,7,6,2),(0,1,5,4)]
obj = "# ARIA original cube prop; edit in any 3D authoring tool\nmtllib cube.mtl\nusemtl Mint\n"
obj += "".join("v %s %s %s\n" % p for p in corners)
obj += "".join("f " + " ".join(str(i+1) for i in f) + "\n" for f in faces)
text(ASSETS / "cube.obj", obj)
text(ASSETS / "cube.mtl", "newmtl Mint\nKd 0.43 0.91 0.81\nKa 0.1 0.1 0.1\nd 1.0\n")
positions = [corners[f[k]] for f in faces for k in [0,1,2,0,2,3]]
binary = b"".join(struct.pack("<3f", *p) for p in positions)
gltf = {"asset":{"version":"2.0","generator":"ARIA original template generator"},
        "scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],
        "buffers":[{"byteLength":len(binary)}],
        "bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":len(binary)}],
        "accessors":[{"bufferView":0,"componentType":5126,"count":len(positions),"type":"VEC3","min":[-.5]*3,"max":[.5]*3}],
        "materials":[{"name":"Mint","pbrMetallicRoughness":{"baseColorFactor":[.43,.91,.81,1],"metallicFactor":0,"roughnessFactor":1}}],
        "meshes":[{"primitives":[{"attributes":{"POSITION":0},"material":0,"mode":4}]}]}
j = json.dumps(gltf, separators=(",", ":")).encode()
j += b" " * (-len(j) % 4)
glb = struct.pack("<3I", 0x46546c67, 2, 12+8+len(j)+8+len(binary))
glb += struct.pack("<2I",len(j),0x4e4f534a)+j+struct.pack("<2I",len(binary),0x004e4942)+binary
(ASSETS / "cube.glb").write_bytes(glb)
gltf["buffers"][0]["uri"] = "cube.bin"
(ASSETS / "cube.bin").write_bytes(binary)
document(ASSETS / "cube.gltf", gltf)
verts = ",".join(str(x) for p in corners for x in p)
indices = ",".join(str(i if k<3 else -i-1) for f in faces for k,i in enumerate(f))
text(ASSETS / "cube.fbx", f'''; FBX 7.4.0 project file - original ARIA static cube
FBXHeaderExtension: {{ FBXHeaderVersion: 1003
 FBXVersion: 7400
 Creator: "ARIA template generator"
}}
GlobalSettings: {{ Version: 1000
 Properties70: {{
  P: "UpAxis", "int", "Integer", "",1
  P: "UpAxisSign", "int", "Integer", "",1
  P: "FrontAxis", "int", "Integer", "",2
  P: "FrontAxisSign", "int", "Integer", "",-1
  P: "CoordAxis", "int", "Integer", "",0
  P: "CoordAxisSign", "int", "Integer", "",1
  P: "UnitScaleFactor", "double", "Number", "",1
 }}
}}
Objects: {{
 Geometry: 100, "Geometry::Cube", "Mesh" {{
  Vertices: *24 {{ a: {verts} }}
  PolygonVertexIndex: *24 {{ a: {indices} }}
  GeometryVersion: 124
 }}
 Model: 200, "Model::Cube", "Mesh" {{ Version: 232
  Properties70: {{
   P: "Lcl Translation", "Lcl Translation", "", "A",0,0,0
   P: "Lcl Rotation", "Lcl Rotation", "", "A",0,0,0
   P: "Lcl Scaling", "Lcl Scaling", "", "A",1,1,1
  }}
 }}
}}
Connections: {{
 C: "OO",100,200
 C: "OO",200,0
}}
''')
with wave.open(str(ASSETS / "pop.wav"), "wb") as f:
    rate, duration = 44100, .18
    f.setparams((1,2,rate,0,"NONE","not compressed"))
    f.writeframes(b"".join(struct.pack("<h", int(6500*math.sin(math.tau*(620*t-900*t*t))*(1-t/duration)**2)) for t in (n/rate for n in range(int(rate*duration)))))

def design(name, kind, assets, **kwargs):
    d = dict(id=1,name=name,kind=kind,assets=assets,selection="Cycle",count=5,interval=.12,
             flight=.65,size=.12,size_variance=.25,origin=[-.8,-.25],target=[0,-.16],spread=.12,
             arc=.18,spin=360,bounce=.6,lifetime=1.4,impact=.18,tint=[255]*4,splash=.65,
             launch_sound="builtin:whoosh",impact_sound="assets/pop.wav",volume=.45,cooldown=.5,hotkey=None)
    d.update(kwargs)
    d.update(routes=[{"origin":d["origin"],"target":d["target"]},
                     {"origin":[.8,-.25],"target":[.08,-.16]}], route_selection="Cycle",
             asset_counts=[d["count"]//len(assets) + int(i<d["count"]%len(assets)) for i in range(len(assets))],
             speed=1, gravity=1, drag=.15, stickiness=1 if kind=="Spray" else .3, fade_out=.7,
             liquid=dict(gloss=.9,clarity=.7,viscosity=.25,drip=.035,foam=.3,trail=.65))
    d["deformation"] = dict(
        avatar=dict(enabled=kind=="Throw",depth=.4,radius=.12,squash=.25,hold=.08,recovery=.8,elasticity=.35,shading=.25),
        object=dict(enabled=kind=="Throw",depth=.4,radius=.8,squash=.5,hold=.08,recovery=.8,elasticity=.35,shading=.25),
        speed_sensitive=False)
    return {"aria_effect":1,"design":d}

document(ROOT / "custom-throw.aria-effect.json", design("My star throw","Throw",["assets/star.png"]))
document(ROOT / "custom-spray.aria-effect.json", design("My water spray","Spray",["builtin:drop"],count=80,interval=.018,size=.023,spin=0,flight=.4,lifetime=3,tint=[100,195,255,205],launch_sound="builtin:spray",impact_sound="",impact=.04))
document(ROOT / "custom-3d.aria-effect.json", design("My 3D volley","Throw",["assets/cube.glb","assets/cube.obj","assets/cube.fbx"],count=3,size=.22))
print("Generated editable PNG/SVG, OBJ/MTL, GLB/glTF/FBX, WAV and design templates in", ROOT)
