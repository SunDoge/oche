"""Require the core Oche-to-Dart mapping to agree under JIT and AOT."""

from pathlib import Path
import subprocess
import tempfile


root = Path(__file__).resolve().parent.parent
manifest = root / "Cargo.toml"
example = root / "examples/dart_mapping.oche"

subprocess.run(
    ["cargo", "build", "--quiet", "--manifest-path", str(manifest), "-p", "oche"],
    check=True,
)
cli = root / "target/debug/oche"

cases = [
    ("scalar_int", ["41"], "42"),
    ("scalar_float", ["1.75"], "3.5"),
    ("scalar_bool", ["true"], "true"),
    ("scalar_string", ["agent"], "agent"),
    ("list_value", [], "42"),
    ("struct_value", ["40", "42"], "42"),
    ("union_value", ["42"], "42"),
    ("result_value", ["0"], "-1"),
    ("result_value", ["42"], "42"),
    ("propagation_value", ["0"], "-1"),
    ("propagation_value", ["41"], "42"),
    ("mutable_value", ["41"], "42"),
    ("async_value", ["40"], "42"),
]


def invoke(command: list[str]) -> str:
    result = subprocess.run(
        command,
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    return result.stdout.strip()


jit_results = {
    (function, tuple(arguments)): invoke(
        [str(cli), "run", str(example), function, *arguments]
    )
    for function, arguments, _ in cases
}

with tempfile.TemporaryDirectory(prefix="oche-dart-mapping-") as task_dir:
    executable = Path(task_dir) / "mapping"
    subprocess.run(
        [str(cli), "build-exe", str(example), "-o", str(executable)],
        capture_output=True,
        text=True,
        timeout=60,
        check=True,
    )
    for function, arguments, expected in cases:
        key = (function, tuple(arguments))
        aot = invoke([str(executable), function, *arguments])
        assert jit_results[key] == expected, (function, "JIT", jit_results[key])
        assert aot == expected, (function, "AOT", aot)
        assert jit_results[key] == aot, (function, jit_results[key], aot)

generated = invoke([str(cli), "emit-dart", str(example)])
for dart_type in ["int", "double", "bool", "String", "List<int>", "Future<int>"]:
    assert dart_type in generated
assert "sealed class OcheEnum0" in generated
assert "final class OcheStruct0" in generated
assert "var l0" in generated
assert "dynamic" not in generated

print("Oche core mapping: Dart VM JIT and AOT parity passed")
