"""End-to-end acceptance for value-level Dart VM comptime evaluation."""

from pathlib import Path
import subprocess
import tempfile


root = Path(__file__).resolve().parent.parent
manifest = root / "Cargo.toml"
example = root / "examples/comptime_values.oche"

subprocess.run(
    ["cargo", "build", "--quiet", "--manifest-path", str(manifest), "-p", "oche"],
    check=True,
)
cli = root / "target/debug/oche"


def invoke(command: list[str], function: str) -> str:
    result = subprocess.run(
        [*command, function],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    return result.stdout.strip()


expected = {
    "capacity_value": "32768",
    "ratio_value": "3.0",
    "enabled_value": "true",
    "label_value": "agent-runtime",
    "scaled_value": "42",
    "other_scaled_value": "30",
    "compiled_values": "[1, 2, 42]",
    "nested_values": "[[1, 2], [1, 2, 3]]",
    "compiled_config_name": "agent",
    "compiled_config_limit": "2",
    "compiled_settings_name": "agent",
    "config_type_name": "Config",
    "returned_type_name": "Config",
    "returned_type_arguments": "1",
    "config_field_count": "2",
    "config_first_field": "name",
}
for function, value in expected.items():
    assert invoke([str(cli), "run", str(example)], function) == value

generated = subprocess.run(
    [str(cli), "emit-dart", str(example)],
    capture_output=True,
    text=True,
    timeout=30,
    check=True,
).stdout
for literal in [
    "return 32768;",
    "return 3.0;",
    "return true;",
    "return 'agent-runtime';",
    "return 42;",
    "return 30;",
    "return [1, 2, 42];",
    "return [[1, 2], [1, 2, 3]];",
    "return (OcheStruct0(f0: 'agent', f1: [1, 2])).ocheField_6e616d65;",
]:
    assert literal in generated
assert "comptime" not in generated

with tempfile.TemporaryDirectory(prefix="oche-comptime-values-aot-") as task_dir:
    executable = Path(task_dir) / "comptime-values"
    subprocess.run(
        [str(cli), "build-exe", str(example), "-o", str(executable)],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    for function, value in expected.items():
        assert invoke([str(executable)], function) == value

print("value-level comptime: literal expansion, JIT, and AOT passed")
