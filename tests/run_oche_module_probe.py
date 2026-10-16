"""Compile a Oche module, load it through Rust, register it, and invoke it."""

import json
from pathlib import Path
import subprocess
import tempfile


root = Path(__file__).resolve().parent.parent
subprocess.run(
    [
        "cargo",
        "build",
        "--manifest-path",
        str(root / "Cargo.toml"),
        "--release",
        "-p",
        "oche-dart-vm",
        "--example",
        "oche_module_probe",
    ],
    check=True,
)
dart = Path(
    subprocess.check_output(["mise", "which", "dart"], text=True).strip()
).resolve()
library = root / "target/release/examples/liboche_module_probe.so"
if not library.exists():
    raise RuntimeError("The current process resolver and runner target Linux")

with tempfile.TemporaryDirectory(prefix="oche-module-") as task_dir:
    directory = Path(task_dir)
    module = directory / "arithmetic.dill"
    agent = directory / "pi_core.dill"
    subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(root / "Cargo.toml"),
            "-p",
            "oche",
            "--",
            "build-module",
            str(root / "examples/arithmetic.oche"),
            "-o",
            str(module),
        ],
        check=True,
    )
    subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(root / "Cargo.toml"),
            "-p",
            "oche",
            "--",
            "build-module",
            str(root / "agent/pi_core.oche"),
            "-o",
            str(agent),
        ],
        check=True,
    )
    host = directory / "host.dill"
    subprocess.run(
        [
            str(dart),
            "compile",
            "kernel",
            str(root / "tests/oche_module_host.dart"),
            "-o",
            str(host),
            "--verbosity=error",
        ],
        check=True,
    )
    result = subprocess.run(
        [str(dart.parent / "dartvm"), str(host), str(library), str(module), str(agent)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    if result.returncode:
        raise RuntimeError(
            f"Dart JIT host exited {result.returncode}:\n{result.stdout}\n{result.stderr}"
        )
    report = json.loads(result.stdout)
    if report["result"] != 42:
        raise RuntimeError(f"unexpected tool result: {report}")
    if report["agent_abi"] != 2 or report["agent_effects"] != [1, 3, 4]:
        raise RuntimeError(f"unexpected agent transitions: {report}")
    print(json.dumps(report, indent=2))
