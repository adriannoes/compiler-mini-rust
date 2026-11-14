"""Compare the Mini-C loop with two Rust builds and save their listings."""

import os
from pathlib import Path
import subprocess
import sys
import tempfile

LAB = Path(__file__).resolve().parent
ROOT = LAB.parents[1]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / "scripts"))
from cargo_artifacts import build_executables

CASES = [(-2, 0), (0, 0), (1, 1), (4, 10), (8, 36)]


def check_result(command, expected, label):
    try:
        result = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=30)
    except subprocess.TimeoutExpired:
        raise SystemExit(f"{label}: the program exceeded 30 seconds.") from None
    if (result.returncode, result.stdout, result.stderr) != (0, expected, b""):
        raise SystemExit(
            f"{label}: expected status 0, stdout {expected!r}, and empty stderr. "
            f"Received status {result.returncode}, stdout {result.stdout!r}, "
            f"and stderr {result.stderr!r}."
        )


def main():
    output = ROOT / "target" / "loop-ir"
    output.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix="run-", dir=output))
    print(f"Lab files: {work}", flush=True)
    (work / "rustc-version.txt").write_bytes(subprocess.check_output(["rustc", "-Vv"], cwd=ROOT))

    minic = build_executables(ROOT, "--bin", "compiler-mini-rust")["compiler-mini-rust"]
    source = LAB / "sum.mc"
    for mode, filename in [("--bytecode", "sum.bytecode"), ("--bytecode-opt", "sum.opt.bytecode")]:
        listing = subprocess.check_output([minic, mode, source], cwd=ROOT)
        (work / filename).write_bytes(listing)

    binaries = []
    for level in (0, 1):
        name = f"sum.O{level}"
        binary = name + (".exe" if os.name == "nt" else "")
        subprocess.run([
            "rustc", "--edition=2024", "--crate-name", "loop_sum", "-C", f"opt-level={level}",
            "-C", "overflow-checks=yes", f"--emit=llvm-ir={name}.ll,link={binary}", LAB / "sum.rs",
        ], cwd=work, check=True)
        binaries.append(work / binary)

    functions = source.read_text(encoding="utf-8")
    for limit, total in CASES:
        case = work / f"limit-{limit}.mc"
        case.write_text(f"{functions}\nint main() {{ return sum_next({limit}); }}\n", encoding="utf-8")
        expected = f"{total}\n".encode()
        for mode in ("--run", "--run-vm", "--run-vm-opt"):
            check_result([minic, mode, case], expected, f"{mode}, limit {limit}")
        for binary in binaries:
            check_result([binary, str(limit)], expected, f"{binary.name}, limit {limit}")
        print(f"limit {limit}: all five runs returned {total}")


if __name__ == "__main__":
    main()
