"""Build Cargo targets and return the executable paths that Cargo reports."""

import json
from pathlib import Path
import subprocess


def build_executables(root, *args):
    output = subprocess.check_output(
        ["cargo", "build", "--locked", "--message-format=json-render-diagnostics", *args],
        cwd=root,
    )
    executables = {}
    for line in output.splitlines():
        message = json.loads(line)
        if message["reason"] == "compiler-artifact" and message["executable"]:
            executables[message["target"]["name"]] = Path(message["executable"])
    return executables
