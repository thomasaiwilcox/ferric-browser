"""Reject unrecorded changes even inside an already patched dependency file."""

import json
import shutil
import tempfile
import unittest
from pathlib import Path

from check_vendor import verify


class VendorPatchTests(unittest.TestCase):
    def test_extra_edit_in_a_documented_modified_file_is_rejected(self):
        vendor = Path(__file__).resolve().parents[1] / "vendor"
        manifest = json.loads((vendor / "SNOW_UPSTREAM.json").read_text())
        with tempfile.TemporaryDirectory(prefix="ferric-vendor-regression-") as temporary:
            source = Path(temporary) / "snow"
            shutil.copytree(vendor / "snow", source)
            changed = source / "src/resolvers/default.rs"
            with changed.open("a") as output:
                output.write("\n// An extra edit outside the recorded patch.\n")
            with self.assertRaisesRegex(SystemExit, "contents do not match"):
                verify(source, manifest, "original_sha256", vendor / "snow-ferric.patch")
