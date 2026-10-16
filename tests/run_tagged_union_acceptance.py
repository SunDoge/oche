"""Exercise tagged unions, exhaustive match, let patterns, try, and propagation."""

from pathlib import Path
import subprocess
import tempfile


root = Path(__file__).resolve().parent.parent
manifest = root / "Cargo.toml"
example = root / "examples/tagged_union_try.oche"

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


assert run("add_positive_value", "2", "3") == "5"
assert run("add_positive_value", "0", "3") == "-1"
assert run("add_present_value", "2", "3") == "5"
assert run("add_present_value", "0", "3") == "-1"
assert run("destructure") == "2"
assert run("render_text", "hello") == "hello"
assert run("render_stop") == "stopped"
assert run("boxed", "42") == "42"

with tempfile.TemporaryDirectory(prefix="oche-tagged-aot-") as task_dir:
    executable = Path(task_dir) / "tagged-union"
    subprocess.run(
        [str(cli), "build-exe", str(example), "-o", str(executable)],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    result = subprocess.run(
        [str(executable), "add_positive_value", "20", "22"],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert result.stdout.strip() == "42"

print("tagged unions + let + try/? propagation: JIT and AOT passed")
