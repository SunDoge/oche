"""Drive the Oche agent machine from TypeScript through one persistent Dart JIT."""

from pathlib import Path
import subprocess


root = Path(__file__).resolve().parent.parent
subprocess.run(
    ["cargo", "build", "--quiet", "--manifest-path", str(root / "Cargo.toml"), "-p", "oche"],
    check=True,
)
result = subprocess.run(
    [
        "node",
        str(root / "tests/run_jit_agent_machine_acceptance.ts"),
        str(root / "target/debug/oche"),
        str(root / "agent/pi_core.oche"),
        str(root / "examples/js_module.oche"),
    ],
    capture_output=True,
    text=True,
    timeout=30,
    check=True,
)
assert (
    result.stdout.strip()
    == "TypeScript drove Oche agent transitions in one persistent Dart JIT process"
)
print("Oche agent machine: persistent TypeScript-to-Dart-JIT transport passed")
