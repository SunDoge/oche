"""Exercise typed list literals under Dart JIT and AOT."""

from pathlib import Path
import subprocess
import tempfile


root = Path(__file__).resolve().parent.parent
manifest = root / "Cargo.toml"
example = root / "examples/lists.oche"

subprocess.run(
    ["cargo", "build", "--quiet", "--manifest-path", str(manifest), "-p", "oche"],
    check=True,
)
cli = root / "target/debug/oche"


def run(function: str, *arguments: str) -> str:
    result = subprocess.run(
        [str(cli), "run", str(example), function, *arguments],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    return result.stdout.strip()


assert run("integers") == "[1, 2, 3]"
assert run("nested") == "[[1], [2, 3]]"
assert run("empty") == "[]"
assert run("last") == "tool"
assert run("classify", "agent") == "1"
assert run("classify", "other") == "0"

with tempfile.TemporaryDirectory(prefix="oche-list-aot-") as task_dir:
    executable = Path(task_dir) / "lists"
    subprocess.run(
        [str(cli), "build-exe", str(example), "-o", str(executable)],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    result = subprocess.run(
        [str(executable), "nested"],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert result.stdout.strip() == "[[1], [2, 3]]"

print("typed lists: Dart JIT and AOT passed")
