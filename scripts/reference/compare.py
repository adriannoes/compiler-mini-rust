"""Build both Mini-C projects and compare their commands and execution limits."""

import argparse
from concurrent.futures import ThreadPoolExecutor
from itertools import product
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
from compare_cli import compare

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
MODES = (
    "--tokens", "--ast", "--resolve", "--check", "--bytecode", "--bytecode-opt",
    "--run", "--run-vm", "--run-vm-opt",
)
PROFILES = ("debug", "release")


def build(reference, work):
    cpp = work / "cpp"
    rust = work / "rust"
    subprocess.run([
        "cmake", "-S", HERE, "-B", cpp, "-G", "Unix Makefiles",
        f"-DREFERENCE_SOURCE={reference}", "-DBUILD_TESTING=ON",
    ], check=True)
    subprocess.run(["cmake", "--build", cpp, "--parallel", "4"], check=True)
    subprocess.run(["ctest", "--test-dir", cpp, "--parallel", "4", "--output-on-failure"],
                   check=True)
    for profile in PROFILES:
        flags = ["--release"] if profile == "release" else []
        for command in ("build", "test"):
            subprocess.run(["cargo", command, "--locked", "--target-dir", rust, *flags],
                           cwd=ROOT, check=True)
        lib = rust / profile
        subprocess.run([
            "rustc", "--edition=2024", HERE / "limits.rs", "--extern",
            f"compiler_mini_rust={lib / 'libcompiler_mini_rust.rlib'}",
            "-L", f"dependency={lib / 'deps'}", "-o", work / f"limits-{profile}",
        ], check=True)


def write_inputs(work, name, cases):
    folder = work / name
    folder.mkdir()
    paths = []
    for title, source in cases.items():
        path = folder / f"{title}.mc"
        path.write_text(source, encoding="ascii")
        paths.append(path)
    return paths


def example_paths(rust_examples, reference_examples):
    references = sorted(reference_examples.glob("*.mc"))
    if not references:
        raise ValueError(f"No .mc examples found in {reference_examples}.")
    for reference in references:
        local = rust_examples / reference.name
        if not local.is_file():
            raise ValueError(f"The Rust checkout has no copy of {reference.name}.")
        if local.read_bytes() != reference.read_bytes():
            raise ValueError(f"The reference and Rust copies of {reference.name} differ.")
    return sorted(rust_examples.glob("*.mc"))


def compare_fixtures(build_dir, executables, work):
    catalog = subprocess.check_output(["ctest", "--test-dir", build_dir, "--show-only=json-v1"])
    cases = [case for case in json.loads(catalog)["tests"] if case["name"].startswith("cli.")]
    if not any(case["name"] == "cli.help" for case in cases):
        raise ValueError("CTest did not register the reference CLI help case.")
    environment = {**os.environ, "MINIC_COMPARE_BINARIES": json.dumps(list(map(str, executables)))}

    def run_case(case):
        command = list(case["command"])
        for index, arg in enumerate(command):
            if arg.startswith("-DMINIC="):
                command[index] = f"-DMINIC={HERE / 'compare_cli.py'}"
                break
        else:
            raise ValueError(f"{case['name']} does not specify a Mini-C executable.")
        # Give each fixture its own source path when CTest cases run concurrently.
        for index, arg in enumerate(command):
            if arg.startswith("-DWORK_DIR="):
                command[index] = f"-DWORK_DIR={work / case['name']}"
                break
        result = subprocess.run(command, env=environment, capture_output=True, timeout=100)
        if result.returncode:
            raise ValueError(f"{case['name']}: {result.stderr.decode(errors='replace')}")

    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(run_case, cases))
    return len(cases)


def compare_limits(work, paths):
    budgets = range(1, 81)
    depths = (1, 2, 4)
    engines = ("interpreter", "VM", "optimized VM")
    scenarios = list(product(paths, budgets, depths))

    def outcomes(probe):
        rows = subprocess.check_output([probe, *paths], timeout=120).splitlines()
        if len(rows) != len(scenarios) * len(engines):
            raise ValueError(f"{probe} returned {len(rows)} engine outcomes.")
        return rows

    expected = outcomes(work / "cpp" / "compare_limits")
    for profile in PROFILES:
        actual = outcomes(work / f"limits-{profile}")
        for index, (path, steps, depth) in enumerate(scenarios):
            start = index * len(engines)
            for offset, engine in enumerate(engines):
                reference = expected[start + offset]
                rust = actual[start + offset]
                if rust != reference or reference != expected[start]:
                    raise ValueError(
                        f"{path.name}, {steps} steps, depth {depth}, {engine}, {profile}: "
                        f"C++ interpreter {expected[start]!r}, C++ {reference!r}, Rust {rust!r}"
                    )
    return len(scenarios)


def validate(reference, work):
    paths = example_paths(ROOT / "examples", reference / "examples")
    build(reference, work)
    cpp = work / "cpp"
    executables = [cpp / "reference" / "minic"]
    executables.extend(work / "rust" / p / "compiler-mini-rust" for p in PROFILES)
    programs = json.loads((HERE / "programs.json").read_text(encoding="ascii"))
    paths.extend(write_inputs(work, "cli", programs["cli"]))
    paths.append(ROOT / "examples")
    commands = [[mode, str(path)] for path in paths for mode in MODES]
    commands.extend([["--help"], ["-h"]])
    for command in commands:
        compare(executables, command)
    fixtures = compare_fixtures(cpp, executables, work)
    limit_paths = [ROOT / "examples" / f"{name}.mc" for name in ("optimization", "arrays", "array_error")]
    limit_paths.extend(write_inputs(work, "limits", programs["limits"]))
    limits = compare_limits(work, limit_paths)
    print(f"Each Rust profile matched {len(commands)} direct CLI calls and {fixtures} reference CLI fixtures.")
    print(f"All three engines matched C++ at {limits} combinations of input, step budget, and call depth per profile.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path, help="Path to the compiler-mini-c source checkout.")
    args = parser.parse_args()
    reference = args.reference.resolve()
    with tempfile.TemporaryDirectory(prefix="minic-reference-") as temp:
        work = Path(temp).resolve()
        if any(root == work or root in work.parents for root in (reference, ROOT)):
            parser.error("Set TMPDIR to a directory outside both checkouts.")
        validate(reference, work)


if __name__ == "__main__":
    main()
