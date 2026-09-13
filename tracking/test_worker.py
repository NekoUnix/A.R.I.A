import math
import unittest
from worker import pose_from_matrix, packet
from nvidia import quaternion_matrix, shapes_from_coefficients


class TrackingTests(unittest.TestCase):
    def test_axes_are_independent_and_consistent_across_backends(self):
        angle = math.radians(20)
        for axis, key in enumerate(['x', 'y', 'z']):
            q = [0., 0., 0., math.cos(angle/2)]
            q[axis] = math.sin(angle/2)
            rotation = pose_from_matrix(quaternion_matrix(q))
            self.assertAlmostEqual(rotation[key], 20., places=5)
            for other in {'x', 'y', 'z'} - {key}:
                self.assertAlmostEqual(rotation[other], 0., places=5)

    def test_nvidia_coefficients_use_the_sdk_order(self):
        coefficients = [0.] * 53
        coefficients[10] = .9
        coefficients[26] = .7
        coefficients[45] = .8
        coefficients[2] = .2
        coefficients[3] = .6
        result = shapes_from_coefficients(coefficients)
        self.assertEqual(result['eyeblinkleft'], .9)
        self.assertEqual(result['jawopen'], .7)
        self.assertEqual(result['mouthsmileleft'], .8)
        self.assertAlmostEqual(result['browinnerup'], .4)

    def test_invalid_native_results_and_face_loss(self):
        with self.assertRaises(ValueError):
            quaternion_matrix([0., 0., 0., 0.])
        with self.assertRaises(ValueError):
            shapes_from_coefficients([float('nan')] * 53)
        lost = packet()
        self.assertFalse(lost['face_found'])
        self.assertEqual(lost['blend_shapes'], {})
        self.assertEqual(lost['rotation'], dict(x=0., y=0., z=0.))


if __name__ == '__main__':
    unittest.main()
