"""Exercise transitive aliased Oche modules through the real Dart toolchain."""

from pathlib import Path
import json
import subprocess
import tempfile


root = Path(__file__).resolve().parent.parent
manifest = root / "Cargo.toml"

subprocess.run(
    ["cargo", "build", "--quiet", "--manifest-path", str(manifest), "-p", "oche"],
    check=True,
)
cli = root / "target/debug/oche"

with tempfile.TemporaryDirectory(prefix="oche-modules-") as task_dir_text:
    task_dir = Path(task_dir_text)
    (task_dir / "math.oche").write_text(
        "pub fn double(value: Int) -> Int { value + value }\n"
        "pub fn maybe(value: Int) -> Option<Int> { Some(value) }\n"
    )
    (task_dir / "service.oche").write_text(
        'import "math.oche" as math\n'
        "fn hidden() -> Int { 99 }\n"
        "pub fn answer() -> Int { math.double(21) }\n"
        "pub fn maybe_answer() -> Option<Int> { math.maybe(answer()) }\n"
    )
    source = task_dir / "main.oche"
    test_source = task_dir / "module_test.oche"
    test_source.write_text(
        'import "service.oche" as service\n'
        "test fn imported_answer() -> Bool {\n"
        "  match service.maybe_answer() { Some(_) => true, None => false }\n"
        "}\n"
    )
    source.write_text(
        'import "service.oche" as service\n'
        'import "math.oche" as math\n'
        "pub fn answer() -> Int { service.answer() }\n"
        "fn direct() -> Int { math.double(20) }\n"
        "fn maybe_answer() -> Int {\n"
        "  match service.maybe_answer() { Some(value) => value, None => 0 }\n"
        "}\n"
    )

    def cli_run(*arguments: str, check: bool = True) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [str(cli), *arguments],
            capture_output=True,
            text=True,
            timeout=30,
            check=check,
        )

    assert cli_run("run", str(source), "answer").stdout.strip() == "42"
    assert cli_run("run", str(source), "direct").stdout.strip() == "40"
    assert cli_run("run", str(source), "maybe_answer").stdout.strip() == "42"

    generated = cli_run("emit-dart", str(source)).stdout
    assert generated.count("int ocheModule2Function0(int p0)") == 1
    assert "return ocheModule1Function1();" in generated

    tested = cli_run("test", str(test_source))
    assert "PASS imported_answer" in tested.stdout
    assert "1 passed; 0 failed" in tested.stdout

    kernel_module = task_dir / "module.dill"
    cli_run("build-module", str(source), "-o", str(kernel_module))
    assert kernel_module.is_file()

    executable = task_dir / "module_app"
    cli_run("build-exe", str(source), "-o", str(executable))
    native = subprocess.run(
        [str(executable), "answer"],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert native.stdout.strip() == "42"

    javascript = task_dir / "module.mjs"
    cli_run("build-js", str(source), "-o", str(javascript))
    node = subprocess.run(
        [
            "node",
            "--input-type=module",
            "-e",
            "const {default: core} = await import(process.argv[1]); console.log(core.answer());",
            javascript.as_uri(),
        ],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert node.stdout.strip() == "42"

    server = subprocess.Popen(
        [str(cli), "serve", str(source)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert server.stdin is not None
    assert server.stdout is not None
    server.stdin.write(json.dumps({"id": 1, "method": "answer", "args": []}) + "\n")
    server.stdin.flush()
    response = json.loads(server.stdout.readline())
    server.stdin.close()
    server.wait(timeout=30)
    assert response == {"id": 1, "ok": True, "value": 42}

    private_source = task_dir / "private.oche"
    private_source.write_text(
        'import "service.oche" as service\n'
        "fn invalid() -> Int { service.hidden() }\n"
    )
    private = cli_run("check", str(private_source), check=False)
    assert private.returncode != 0
    assert "Unknown qualified function 'service.hidden'" in private.stderr

    (task_dir / "cycle_a.oche").write_text(
        'import "cycle_b.oche" as b\npub fn value() -> Int { b.value() }\n'
    )
    (task_dir / "cycle_b.oche").write_text(
        'import "cycle_a.oche" as a\npub fn value() -> Int { a.value() }\n'
    )
    cycle = cli_run("check", str(task_dir / "cycle_a.oche"), check=False)
    assert cycle.returncode != 0
    assert "Circular Oche module import:" in cycle.stderr
    assert "cycle_a.oche" in cycle.stderr and "cycle_b.oche" in cycle.stderr

print(
    "transitive Oche modules: JIT, AOT, Kernel module, JavaScript, tests, RPC, "
    "visibility, and cycle checks passed"
)
