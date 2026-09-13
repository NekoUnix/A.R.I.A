"""Owned, local camera worker. Only numeric tracking leaves this process.

Stdout is a bounded JSON-lines protocol; diagnostics go to stderr. No video is
recorded or sent to a server. See docs/webcam.md for setup and SDK licensing.
"""
import argparse
import json
import math
import sys
import time


def pose_from_matrix(m):
    return dict(x=math.degrees(math.atan2(m[2][1], m[2][2])),
                y=math.degrees(math.atan2(-m[2][0], math.hypot(m[0][0], m[1][0]))),
                z=math.degrees(math.atan2(m[1][0], m[0][0])))


def packet(rotation=None, shapes=None, found=False):
    return dict(version=1, timestamp=int(time.time()*1000), face_found=found,
                rotation=rotation or dict(x=0., y=0., z=0.),
                position=dict(x=0., y=0., z=0.), eye_left=dict(x=0., y=0., z=0.),
                eye_right=dict(x=0., y=0., z=0.), blend_shapes=shapes or {}, hotkey=0)


class MediaPipe:
    def __init__(self, args):
        import mediapipe as mp
        self.mp = mp
        options = mp.tasks.vision.FaceLandmarkerOptions(
            base_options=mp.tasks.BaseOptions(model_asset_path=args.model),
            running_mode=mp.tasks.vision.RunningMode.VIDEO,
            num_faces=1, output_face_blendshapes=True,
            output_facial_transformation_matrixes=True,
            min_face_detection_confidence=args.confidence,
            min_face_presence_confidence=args.confidence,
            min_tracking_confidence=args.confidence)
        self.engine = mp.tasks.vision.FaceLandmarker.create_from_options(options)
        self.timestamp = -1

    def track(self, bgr):
        import cv2
        rgb = cv2.cvtColor(bgr, cv2.COLOR_BGR2RGB)
        image = self.mp.Image(image_format=self.mp.ImageFormat.SRGB, data=rgb)
        self.timestamp = max(self.timestamp+1, int(time.monotonic()*1000))
        result = self.engine.detect_for_video(image, self.timestamp)
        if not result.face_landmarks or not result.facial_transformation_matrixes:
            return packet()
        shapes = {x.category_name.lower(): max(0., min(1., x.score))
                  for x in result.face_blendshapes[0] if x.category_name != '_neutral'}
        return packet(pose_from_matrix(result.facial_transformation_matrixes[0]), shapes, True)

    def close(self):
        self.engine.close()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--backend', choices=['mediapipe', 'nvidia'], default='mediapipe')
    parser.add_argument('--device', type=int, default=0)
    parser.add_argument('--width', type=int, default=640)
    parser.add_argument('--height', type=int, default=480)
    parser.add_argument('--fps', type=int, default=30)
    parser.add_argument('--confidence', type=float, default=.5)
    parser.add_argument('--model', default='face_landmarker.task')
    parser.add_argument('--sdk', default='')
    parser.add_argument('--image', help='Offline inference test; does not open the camera')
    parser.add_argument('--list', action='store_true')
    args = parser.parse_args()
    import cv2
    if args.list:
        # DirectShow names and indices use the same Windows device ordering.
        from cv2_enumerate_cameras import enumerate_cameras
        print(json.dumps([dict(index=c.index, name=c.name) for c in enumerate_cameras(cv2.CAP_DSHOW)]))
        return
    if not (0 <= args.device < 64 and 160 <= args.width <= 1920 and
            120 <= args.height <= 1080 and 5 <= args.fps <= 60 and .1 <= args.confidence <= .99):
        raise ValueError('Invalid camera settings')
    if args.backend == 'nvidia':
        from nvidia import Nvidia
        engine = Nvidia(args)
    else:
        engine = MediaPipe(args)
    camera = None
    try:
        if args.image:
            image = cv2.imread(args.image)
            if image is None:
                raise ValueError('Cannot read test image')
            print(json.dumps(engine.track(image), allow_nan=False), flush=True)
            return
        camera = cv2.VideoCapture(args.device, cv2.CAP_DSHOW if sys.platform == 'win32' else cv2.CAP_ANY)
        if not camera.isOpened():
            raise RuntimeError('Camera unavailable. Check Windows camera privacy, select another device, and close apps using it.')
        camera.set(cv2.CAP_PROP_FRAME_WIDTH, args.width)
        camera.set(cv2.CAP_PROP_FRAME_HEIGHT, args.height)
        camera.set(cv2.CAP_PROP_FPS, args.fps)
        camera.set(cv2.CAP_PROP_BUFFERSIZE, 1)
        print(f'{args.backend}: camera {args.device}, {int(camera.get(3))} x {int(camera.get(4))}', file=sys.stderr, flush=True)
        while True:
            started = time.monotonic()
            ok, bgr = camera.read()
            if not ok:
                raise RuntimeError('Camera disconnected or stopped providing frames. Reconnect and press Start camera.')
            print(json.dumps(engine.track(bgr), allow_nan=False, separators=(',', ':')), flush=True)
            time.sleep(max(0., 1./args.fps-(time.monotonic()-started)))
    finally:
        if camera is not None:
            camera.release()
        engine.close()


if __name__ == '__main__':
    try:
        main()
    except (BrokenPipeError, KeyboardInterrupt):
        pass
    except Exception as error:
        print(f'Tracking stopped: {error}', file=sys.stderr, flush=True)
        sys.exit(1)
