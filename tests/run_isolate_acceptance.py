"""Exercise std/isolate through Dart JIT and AOT."""

from pathlib import Path
import subprocess
import tempfile


root = Path(__file__).resolve().parent.parent
manifest = root / "Cargo.toml"
example = root / "examples/isolate.oche"

subprocess.run(
    ["cargo", "build", "--quiet", "--manifest-path", str(manifest), "-p", "oche"],
    check=True,
)
cli = root / "target/debug/oche"

result = subprocess.run(
    [str(cli), "run", str(example), "isolatedDouble", "21"],
    capture_output=True,
    text=True,
    timeout=30,
    check=True,
)
assert result.stdout.strip() == "42"

with tempfile.TemporaryDirectory(prefix="oche-isolate-aot-") as task_dir:
    executable = Path(task_dir) / "isolate"
    subprocess.run(
        [str(cli), "build-exe", str(example), "-o", str(executable)],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    result = subprocess.run(
        [str(executable), "isolatedDouble", "21"],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert result.stdout.strip() == "42"

print("std/isolate: Dart JIT and AOT passed")
