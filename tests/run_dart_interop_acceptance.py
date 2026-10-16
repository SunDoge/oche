"""Exercise generated Dart definitions and namespaced calls in JIT and AOT."""

from pathlib import Path
import subprocess
import tempfile


root = Path(__file__).resolve().parent.parent
manifest = root / "Cargo.toml"
fixture = root / "tests/interop_fixture.dart"

subprocess.run(
    ["cargo", "build", "--quiet", "--manifest-path", str(manifest), "-p", "oche"],
    check=True,
)
cli = root / "target/debug/oche"

with tempfile.TemporaryDirectory(prefix="oche-interop-") as task_dir_text:
    task_dir = Path(task_dir_text)
    definitions = task_dir / "tools.d.oche"
    generated = subprocess.run(
        [str(cli), "bindgen-dart", str(fixture), "-o", str(definitions)],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert "5 function definition(s)" in generated.stdout
    definition_text = definitions.read_text()
    assert "fn greet(name: String) -> String" in definition_text
    assert "fn add(left: Int, right: Int) -> Int" in definition_text
    assert "fn scale(value: Float, factor: Float) -> Float" in definition_text
    assert "fn later(value: String) -> Future<String>" in definition_text

    source = task_dir / "agent.oche"
    source.write_text(
        '''import "tools.d.oche" as tools

fn greet(name: String) -> String { tools.greet(name) }
fn add(left: Int, right: Int) -> Int { tools.add(left, right) }
fn scale(value: Float, factor: Float) -> Float { tools.scale(value, factor) }
fn invert(value: Bool) -> Bool { tools.invert(value) }
async fn later(value: String) -> String { tools.later(value).await }
'''
    )

    def jit(*arguments: str) -> str:
        result = subprocess.run(
            [str(cli), "run", str(source), *arguments],
            capture_output=True,
            text=True,
            timeout=30,
            check=True,
        )
        return result.stdout.strip()

    assert jit("greet", "agent") == "hello, agent"
    assert jit("add", "20", "22") == "42"
    assert jit("scale", "1.5", "2.5") == "3.75"
    assert jit("invert", "true") == "false"
    assert jit("later", "agent") == "later: agent"

    bad_source = task_dir / "unqualified.oche"
    bad_source.write_text(
        'import "tools.d.oche" as tools\n'
        "fn hello(name: String) -> String { greet(name) }\n"
    )
    bad = subprocess.run(
        [str(cli), "check", str(bad_source)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert bad.returncode != 0
    assert "Unknown function 'greet'" in bad.stderr

    executable = task_dir / "agent"
    subprocess.run(
        [str(cli), "build-exe", str(source), "-o", str(executable)],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    result = subprocess.run(
        [str(executable), "later", "agent"],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert result.stdout.strip() == "later: agent"

print("namespaced Dart interop: bindgen, JIT, AOT, and namespace isolation passed")
