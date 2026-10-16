"""Call the host operating system through dart:ffi from Oche in JIT and AOT."""

from pathlib import Path
import subprocess
import tempfile


root = Path(__file__).resolve().parent.parent
manifest = root / "Cargo.toml"
wrapper = root / "tests/system_ffi.dart"

subprocess.run(
    ["cargo", "build", "--quiet", "--manifest-path", str(manifest), "-p", "oche"],
    check=True,
)
cli = root / "target/debug/oche"


def assert_process_id(command: list[str]) -> None:
    result = subprocess.run(
        command,
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    process_id = int(result.stdout.strip())
    assert process_id > 0


with tempfile.TemporaryDirectory(prefix="oche-system-ffi-") as task_dir_text:
    task_dir = Path(task_dir_text)
    definitions = task_dir / "system.d.oche"
    generated = subprocess.run(
        [str(cli), "bindgen-dart", str(wrapper), "-o", str(definitions)],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert "1 function definition(s)" in generated.stdout
    assert "fn systemProcessId() -> Int" in definitions.read_text()

    source = task_dir / "system_probe.oche"
    source.write_text(
        'import "system.d.oche" as system\n\n'
        "fn processId() -> Int { system.systemProcessId() }\n"
    )

    assert_process_id([str(cli), "run", str(source), "processId"])

    executable = task_dir / "system_probe"
    subprocess.run(
        [str(cli), "build-exe", str(source), "-o", str(executable)],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert_process_id([str(executable), "processId"])

print("system dart:ffi: getpid passed in Dart JIT and AOT")
