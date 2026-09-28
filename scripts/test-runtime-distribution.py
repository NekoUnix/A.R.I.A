"""Regression tests for the release-content gate; no models or SDKs needed."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("audit", Path(__file__).with_name("check-runtime-distribution.py"))
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


class DistributionTests(unittest.TestCase):
    def test_empty_stage_is_valid(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(audit.check(directory), [])

    def test_nested_sdk_old_core_and_avatar_files_are_rejected(self):
        for name in ["Live2DCubismCore.dll", "libLive2DCubismCore.a", "libLive2DCubismCore.so.6",
                     "Live2DCubismCore.bundle", "PurismCoreBundle.h", "private.MOC3", "avatar.model3.json"]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                stage = Path(directory)
                asset = stage / "unexpected" / name
                asset.parent.mkdir()
                asset.write_bytes(b"fixture")
                self.assertTrue(any("Prohibited" in error for error in audit.check(stage)))


if __name__ == "__main__":
    unittest.main()
