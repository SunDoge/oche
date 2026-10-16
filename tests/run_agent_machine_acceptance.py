"""Build the Oche scheduling kernel and drive it from TypeScript."""

from pathlib import Path
import subprocess
import tempfile


root = Path(__file__).resolve().parent.parent
manifest = root / "Cargo.toml"
source = root / "agent/pi_core.oche"
driver = root / "tests/run_agent_machine_acceptance.ts"

subprocess.run(
    ["cargo", "build", "--quiet", "--manifest-path", str(manifest), "-p", "oche"],
    check=True,
)
cli = root / "target/debug/oche"

with tempfile.TemporaryDirectory(prefix="oche-agent-machine-") as task_dir_text:
    module = Path(task_dir_text) / "pi-core.mjs"
    subprocess.run(
        [str(cli), "build-js", str(source), "-o", str(module)],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    result = subprocess.run(
        ["node", str(driver), module.as_uri()],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert result.stdout.strip() == "TypeScript-driven Oche agent state machine passed"

print("Oche agent machine: TypeScript host transitions passed")
