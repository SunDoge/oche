"""Compile Oche to an ESM module and call synchronous and async exports in Node."""

from pathlib import Path
import subprocess
import tempfile


root = Path(__file__).resolve().parent.parent
manifest = root / "Cargo.toml"
example = root / "examples/js_module.oche"

subprocess.run(
    ["cargo", "build", "--quiet", "--manifest-path", str(manifest), "-p", "oche"],
    check=True,
)
cli = root / "target/debug/oche"

with tempfile.TemporaryDirectory(prefix="oche-js-module-") as task_dir_text:
    module = Path(task_dir_text) / "core.mjs"
    subprocess.run(
        [str(cli), "build-js", str(example), "-o", str(module)],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    script = """
const { default: core } = await import(process.argv[1]);
const values = [
  core.add(20, 22),
  core.greet("agent"),
  core.echo_bool(true),
  core.scale(1.5, 2.5),
  await core.later(41),
  await core.later_float(1.25),
];
console.log(JSON.stringify(values));
"""
    result = subprocess.run(
        ["node", "--input-type=module", "-e", script, module.as_uri()],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert result.stdout.strip() == '[42,"hello, agent",true,3.75,42,1.75]'

print("JavaScript module exports: synchronous and async calls passed")
