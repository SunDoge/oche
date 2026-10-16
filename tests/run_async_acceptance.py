"""Exercise async/await and async try propagation under Dart JIT and AOT."""

from pathlib import Path
import subprocess
import tempfile


root = Path(__file__).resolve().parent.parent
manifest = root / "Cargo.toml"
example = root / "examples/async.oche"

subprocess.run(
    ["cargo", "build", "--quiet", "--manifest-path", str(manifest), "-p", "oche"],
    check=True,
)
cli = root / "target/debug/oche"


def run(*arguments: str) -> str:
    result = subprocess.run(
        [str(cli), "run", str(example), *arguments],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    return result.stdout.strip()


assert run("chained", "40") == "42"
assert run("propagated_value", "41") == "42"

with tempfile.TemporaryDirectory(prefix="oche-async-aot-") as task_dir:
    executable = Path(task_dir) / "async"
    subprocess.run(
        [str(cli), "build-exe", str(example), "-o", str(executable)],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    result = subprocess.run(
        [str(executable), "propagated_value", "41"],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert result.stdout.strip() == "42"

print("async/await: Dart JIT and AOT passed")
