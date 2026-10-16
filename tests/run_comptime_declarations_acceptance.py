"""End-to-end acceptance for Oche-generated typed declarations."""

from pathlib import Path
import subprocess
import tempfile


root = Path(__file__).resolve().parent.parent
manifest = root / "Cargo.toml"
example = root / "examples/comptime_declarations.oche"

subprocess.run(
    ["cargo", "build", "--quiet", "--manifest-path", str(manifest), "-p", "oche"],
    check=True,
)
cli = root / "target/debug/oche"


def invoke(command: list[str], function: str = "generated_answer") -> str:
    result = subprocess.run(
        [*command, function],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    return result.stdout.strip()


assert invoke([str(cli), "run", str(example)]) == "42"
assert invoke([str(cli), "run", str(example)], "use_generated") == "42"
generated = subprocess.run(
    [str(cli), "emit-dart", str(example)],
    capture_output=True,
    text=True,
    timeout=30,
    check=True,
).stdout
assert "return 42;" in generated
assert "ComptimeDeclarations" not in generated

with tempfile.TemporaryDirectory(prefix="oche-comptime-declarations-aot-") as task_dir:
    executable = Path(task_dir) / "comptime-declarations"
    subprocess.run(
        [str(cli), "build-exe", str(example), "-o", str(executable)],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert invoke([str(executable)]) == "42"
    assert invoke([str(executable)], "use_generated") == "42"

print("comptime declarations: Oche generation, JIT, and AOT passed")
