"""End-to-end acceptance for the built-in JSON namespace."""

from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parent.parent
manifest = root / "Cargo.toml"
example = root / "examples/json_namespace.oche"
subprocess.run(
    ["cargo", "build", "--quiet", "--manifest-path", str(manifest), "-p", "oche"],
    check=True,
)
cli = root / "target/debug/oche"


def invoke(command: list[str], function: str, *arguments: str) -> str:
    result = subprocess.run(
        [*command, function, *arguments],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    return result.stdout.strip()


expected = '{"name":"Ada","age":37,"active":true}'
command = [str(cli), "run", str(example)]
assert invoke(command, "encode_user", "Ada", "37", "true") == expected
assert invoke(command, "encode_user", 'A"\n\\', "37", "true") == (
    '{"name":"A\\"\\n\\\\","age":37,"active":true}'
)
assert invoke(command, "round_trip", expected) == expected
assert invoke(
    command,
    "round_trip",
    '{"ignored":{"nested":[1,true,null]},"active":true,"age":37,"name":"Ada"}',
) == expected
assert invoke(command, "round_trip", '{"name":"Ada","age":37') == "Invalid JSON"
assert invoke(command, "round_trip", '{"name":"Ada","age":01,"active":true}') == (
    "Invalid JSON"
)
assert invoke(command, "through_value", expected) == expected
assert invoke(command, "dynamic_round_trip", '{"a":[1,true,null]}') == (
    '{"a":[1,true,null]}'
)
assert invoke(command, "dynamic_name", expected) == "Ada"
assert invoke(command, "dynamic_name", '{"name":42}') == "Expected a JSON string"

generated = subprocess.run(
    [str(cli), "emit-dart", str(example)],
    capture_output=True,
    text=True,
    timeout=30,
    check=True,
).stdout
assert "std/json" not in generated
assert "User_toJson" not in generated
assert "_oche_generate_json" not in generated
assert "Object? ocheJsonToValue0" in generated
assert "final f0 = _ocheJsonString" in generated
assert "StringBuffer" in generated
assert "jsonEncode(ocheJsonToValue0(value))" not in generated
assert "_OcheJsonReader" in generated
assert "final value = _ocheJsonRead0(reader" in generated

with tempfile.TemporaryDirectory(prefix="oche-json-namespace-aot-") as task_dir:
    executable = Path(task_dir) / "oche-json"
    subprocess.run(
        [str(cli), "build-exe", str(example), "-o", str(executable)],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert invoke([str(executable)], "round_trip", expected) == expected
    assert invoke([str(executable)], "through_value", expected) == expected
    assert invoke([str(executable)], "dynamic_name", expected) == "Ada"

print("JSON namespace: typed and dynamic conversion, JIT, and AOT passed")
