"""Exercise bounded expression generation through Dart JIT and AOT."""

from pathlib import Path
import subprocess
import tempfile


root = Path(__file__).resolve().parent.parent
manifest = root / "Cargo.toml"
example = root / "examples/generated_expressions.oche"

subprocess.run(
    ["cargo", "build", "--quiet", "--manifest-path", str(manifest), "-p", "oche"],
    check=True,
)
cli = root / "target/debug/oche"


def run(function: str) -> str:
    result = subprocess.run(
        [str(cli), "run", str(example), function, "21"],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    return result.stdout.strip()


assert run("generated") == "43"
assert run("generatedBlock") == "43"
assert run("storedCode") == "43"
assert run("reusedCode") == "44"
assert run("spliceScope") == "43"

generated = subprocess.run(
    [str(cli), "emit-dart", str(example)],
    capture_output=True,
    text=True,
    timeout=30,
    check=True,
).stdout
assert "quote" not in generated
assert "splice quote" not in generated

with tempfile.TemporaryDirectory(prefix="oche-generated-expression-aot-") as task_dir:
    executable = Path(task_dir) / "generated-expressions"
    subprocess.run(
        [str(cli), "build-exe", str(example), "-o", str(executable)],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    result = subprocess.run(
        [str(executable), "storedCode", "21"],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert result.stdout.strip() == "43"

print("generated expressions: Dart JIT and AOT passed")
