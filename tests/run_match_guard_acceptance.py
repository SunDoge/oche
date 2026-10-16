"""Exercise typed match guards through the Dart JIT and AOT backends."""

from pathlib import Path
import subprocess
import tempfile


root = Path(__file__).resolve().parent.parent
manifest = root / "Cargo.toml"
example = root / "examples/match_guards.oche"

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


assert run("guarded") == "11"
assert run("fallback") == "0"
assert run("absent") == "-1"

with tempfile.TemporaryDirectory(prefix="oche-match-guard-aot-") as task_dir:
    executable = Path(task_dir) / "match-guards"
    subprocess.run(
        [str(cli), "build-exe", str(example), "-o", str(executable)],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    result = subprocess.run(
        [str(executable), "guarded"],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert result.stdout.strip() == "11"

print("typed match guards: JIT and AOT passed")
