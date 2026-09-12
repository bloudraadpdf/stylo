# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class OutputPathTests(unittest.TestCase):
    def test_documentation_from_extended_output_path(self):
        target = Path(__file__).resolve().parents[2] / "target"
        target.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=target) as temporary:
            root = os.path.abspath(temporary)
            if os.name == "nt":
                root = "\\\\?\\" + root
            output = os.path.join(root, "target", "debug", "build", "style", "out")
            os.makedirs(output)
            subprocess.run(
                [sys.executable, str(Path(__file__).with_name("build.py")), "servo"],
                env={**os.environ, "OUT_DIR": output},
                check=True,
            )
            for name in ["css-properties.html", "css-properties.json"]:
                self.assertEqual(
                    (Path(temporary) / "target/doc/stylo" / name).read_bytes(),
                    (Path(output) / name).read_bytes(),
                )


if __name__ == "__main__":
    unittest.main()
