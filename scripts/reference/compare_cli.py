#!/usr/bin/env python3
"""Compare CLI results and forward the reference output to a CTest fixture."""

import json
import os
import subprocess
import sys


def compare(executables, arguments):
    def run(executable):
        result = subprocess.run([executable, *arguments], capture_output=True, timeout=30)
        return result.returncode, result.stdout, result.stderr

    expected = run(executables[0])
    for executable in executables[1:]:
        actual = run(executable)
        for name, reference, rust in zip(("exit status", "stdout", "stderr"), expected, actual):
            if reference != rust:
                raise ValueError(
                    f"{executable} {arguments}: {name} differs.\n"
                    f"  C++:  {reference!r}\n  Rust: {rust!r}"
                )
    return expected


if __name__ == "__main__":
    code, stdout, stderr = compare(json.loads(os.environ["MINIC_COMPARE_BINARIES"]), sys.argv[1:])
    sys.stdout.buffer.write(stdout)
    sys.stderr.buffer.write(stderr)
    sys.exit(code)
