"""NVIDIA FaceExpressions adapter. Never falls back to CPU behind an RTX label."""
import ctypes as C
import math
import os
from pathlib import Path
from worker import packet, pose_from_matrix

# SDK 53-coefficient order, as documented by NVIDIA's ExpressionApp sample.
NAMES = '''browDownLeft browDownRight browInnerUpLeft browInnerUpRight browOuterUpLeft browOuterUpRight
cheekPuffLeft cheekPuffRight cheekSquintLeft cheekSquintRight eyeBlinkLeft eyeBlinkRight
eyeLookDownLeft eyeLookDownRight eyeLookInLeft eyeLookInRight eyeLookOutLeft eyeLookOutRight
eyeLookUpLeft eyeLookUpRight eyeSquintLeft eyeSquintRight eyeWideLeft eyeWideRight
jawForward jawLeft jawOpen jawRight mouthClose mouthDimpleLeft mouthDimpleRight mouthFrownLeft mouthFrownRight
mouthFunnel mouthLeft mouthLowerDownLeft mouthLowerDownRight mouthPressLeft mouthPressRight mouthPucker mouthRight
mouthRollLower mouthRollUpper mouthShrugLower mouthShrugUpper mouthSmileLeft mouthSmileRight mouthStretchLeft mouthStretchRight
mouthUpperUpLeft mouthUpperUpRight noseSneerLeft noseSneerRight'''.lower().split()


def shapes_from_coefficients(values):
    if len(values) != 53 or not all(math.isfinite(v) for v in values):
        raise ValueError('Invalid NVIDIA expression coefficients')
    shapes = {name: max(0., min(1., float(v))) for name, v in zip(NAMES, values)}
    shapes['browinnerup'] = (shapes['browinnerupleft'] + shapes['browinnerupright']) / 2
    shapes['cheekpuff'] = (shapes['cheekpuffleft'] + shapes['cheekpuffright']) / 2
    return shapes


def quaternion_matrix(q):
    x, y, z, w = q
    norm = math.sqrt(sum(v*v for v in q))
    if not math.isfinite(norm) or norm < .01:
        raise ValueError('Invalid NVIDIA pose quaternion')
    x, y, z, w = (v/norm for v in (x, y, z, w))
    return [[1-2*(y*y+z*z), 2*(x*y-z*w), 2*(x*z+y*w)],
            [2*(x*y+z*w), 1-2*(x*x+z*z), 2*(y*z-x*w)],
            [2*(x*z-y*w), 2*(y*z+x*w), 1-2*(x*x+y*y)]]


class Nvidia:
    def __init__(self, args):
        if os.name != 'nt':
            raise RuntimeError('The NVIDIA backend currently requires Windows x64')
        root = Path(args.sdk)
        if not args.sdk or not root.is_dir():
            raise RuntimeError('Choose your NVIDIA AR SDK root. Install its FaceExpressions, LandmarkDetection and FaceBoxDetection features first.')
        bridge = root / 'aria-nvar.dll'
        if not bridge.is_file():
            raise RuntimeError('ARIA NVIDIA bridge missing. Run scripts/build-nvidia-bridge.ps1 with your SDK folder; see docs/webcam.md.')
        # Retain handles for the worker lifetime. This changes only this process.
        dirs = sorted({p.parent.resolve() for p in root.rglob('*.dll')})
        if len(dirs) > 128:
            raise RuntimeError('Too many SDK library folders; choose the SDK root only')
        self.dirs = [os.add_dll_directory(str(p)) for p in dirs]
        os.environ['PATH'] = os.pathsep.join(str(p) for p in dirs) + os.pathsep + os.environ.get('PATH', '')
        os.environ['NV_AR_SDK_PATH'] = 'USE_APP_PATH'
        models = next((p for p in [root/'bin'/'models', root/'models'] if p.is_dir()), None)
        self.models = str(models.resolve()).encode('utf-8') if models else b''
        self.dll = C.CDLL(str(bridge.resolve()))
        self.dll.aria_nvar_create.argtypes = [C.c_uint, C.c_uint, C.c_char_p]
        self.dll.aria_nvar_create.restype = C.c_void_p
        self.dll.aria_nvar_error.restype = C.c_char_p
        self.dll.aria_nvar_destroy.argtypes = [C.c_void_p]
        self.dll.aria_nvar_destroy.restype = None
        self.dll.aria_nvar_track.argtypes = [C.c_void_p, C.c_void_p, C.c_uint, C.c_uint, C.c_uint, C.POINTER(C.c_float), C.c_uint]
        self.dll.aria_nvar_track.restype = C.c_int
        self.handle = None
        self.size = None
        self.output = (C.c_float * 57)()

    def error(self):
        return self.dll.aria_nvar_error().decode('utf-8', errors='replace')

    def track(self, bgr):
        import numpy as np
        bgr = np.ascontiguousarray(bgr)
        h, w = bgr.shape[:2]
        if self.size != (w, h):
            if self.handle:
                self.dll.aria_nvar_destroy(self.handle)
            self.handle = self.dll.aria_nvar_create(w, h, self.models)
            if not self.handle:
                raise RuntimeError(self.error())
            self.size = (w, h)
        result = self.dll.aria_nvar_track(self.handle, bgr.ctypes.data, w, h, bgr.strides[0], self.output, 57)
        if result < 0:
            raise RuntimeError(self.error())
        if result == 0:
            return packet()
        return packet(pose_from_matrix(quaternion_matrix(self.output[:4])),
                      shapes_from_coefficients(self.output[4:]), True)

    def close(self):
        if self.handle:
            self.dll.aria_nvar_destroy(self.handle)
            self.handle = None
