"""Regression tests for the release-content gate; no models or SDKs needed."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("audit", Path(__file__).with_name("check-runtime-distribution.py"))
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


class DistributionTests(unittest.TestCase):
    def test_missing_or_modified_notice_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            stage = Path(directory)
            self.assertTrue(audit.check(stage))
            notice = stage / audit.NOTICE
            notice.parent.mkdir(parents=True)
            notice.write_bytes((audit.ROOT / audit.NOTICE).read_bytes())
            self.assertEqual(audit.check(stage), [])
            notice.write_text("MIT without the required copyright notice")
            self.assertTrue(audit.check(stage))

    def test_nested_sdk_and_avatar_files_are_rejected(self):
        for name in ["Live2DCubismCore.dll", "libLive2DCubismCore.a", "libLive2DCubismCore.so.6",
                     "Live2DCubismCore.bundle", "private.MOC3", "avatar.model3.json"]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                stage = Path(directory)
                notice = stage / audit.NOTICE
                notice.parent.mkdir(parents=True)
                notice.write_bytes((audit.ROOT / audit.NOTICE).read_bytes())
                asset = stage / "unexpected" / name
                asset.parent.mkdir()
                asset.write_bytes(b"fixture")
                self.assertTrue(any("Prohibited" in error for error in audit.check(stage)))


if __name__ == "__main__":
    unittest.main()
