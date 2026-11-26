#!/usr/bin/env python3
"""Check the missing Vulkan driver message. Run under Xvfb with the built app path."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile


app = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="aztek_vulkan_") as directory:
    work = Path(directory)
    env = dict(os.environ, AZTEK_ECHO="1", AZTEK_PREFS=str(work / "prefs"),
               AZTEK_APP_DIR=str(work / "aztek"), MAYA_APP_DIR=str(work / "maya"),
               XDG_CONFIG_HOME=str(work / "config"), VK_DRIVER_FILES=str(work / "missing.json"))
    result = subprocess.run([str(app), "--screenshot", str(work / "viewport.png")], env=env,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=30)
    assert result.returncode == 0, result.stdout
    assert "The viewport needs a Vulkan-compatible graphics driver." in result.stdout, result.stdout
print("Missing Vulkan driver message passed.")
