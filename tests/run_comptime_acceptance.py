"""End-to-end acceptance for comptime JSON serde and validation."""

from pathlib import Path
import subprocess
import tempfile


root = Path(__file__).resolve().parent.parent
manifest = root / "Cargo.toml"
example = root / "examples/comptime_json.oche"

subprocess.run(
    ["cargo", "build", "--quiet", "--manifest-path", str(manifest), "-p", "oche"],
    check=True,
)
cli = root / "target/debug/oche"


def cli_run(*arguments: str) -> str:
    result = subprocess.run(
        [str(cli), "run", str(example), *arguments],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    return result.stdout.strip()


expected = '{"name":"Ada","age":37}'
assert cli_run("encode_user", "Ada", "37") == expected
assert cli_run("round_trip", expected) == expected
assert cli_run("round_trip", '{"name":7,"age":37}') == (
    "Expected a JSON string"
)
assert cli_run("validate_user", "Ada", "37") == expected
assert cli_run("validate_user", "", "37") == "name must not be empty"
assert cli_run("validate_user", "Ada", "200") == "age must be between 0 and 130"
assert cli_run("struct_name", "Ada", "37") == "Ada"
assert cli_run("struct_age", "Ada", "37") == "37"
batch = '{"counts":[1,2,3],"labels":[["agent"],["tool","result"]]}'
assert cli_run("round_trip_batch", batch) == batch
assert cli_run("round_trip_batch", '{"counts":[1,true],"labels":[]}') == (
    "Expected a JSON integer"
)
assert cli_run("round_trip_batch", '{"counts":[],"labels":[[1]]}') == (
    "Expected a JSON string"
)
envelope = (
    '{"owner":{"name":"Ada","age":37},'
    '"batches":[{"counts":[1,2],"labels":[["agent"]]}]}'
)
assert cli_run("round_trip_envelope", envelope) == envelope
assert cli_run(
    "round_trip_envelope",
    '{"owner":{"name":"Ada","age":"old"},"batches":[]}',
) == "Expected a JSON integer"

with tempfile.TemporaryDirectory(prefix="oche-comptime-aot-") as task_dir:
    executable = Path(task_dir) / "comptime-json"
    subprocess.run(
        [str(cli), "build-exe", str(example), "-o", str(executable)],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    result = subprocess.run(
        [str(executable), "round_trip", expected],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert result.stdout.strip() == expected

    result = subprocess.run(
        [str(executable), "round_trip_batch", batch],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert result.stdout.strip() == batch

    result = subprocess.run(
        [str(executable), "round_trip_envelope", envelope],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert result.stdout.strip() == envelope

print("comptime JSON serde + validate: JIT and AOT passed")
