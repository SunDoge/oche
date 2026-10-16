"""Compile the Oche scheduler and inject it into a local pi-agent-core checkout."""

import argparse
import os
from pathlib import Path
import subprocess
import tempfile


parser = argparse.ArgumentParser()
parser.add_argument(
    "--update-vendored",
    action="store_true",
    help="replace pi-agent-core's vendored kernel with the current compiler output",
)
args = parser.parse_args()

root = Path(__file__).resolve().parent.parent
default_pi = root.parents[2] / "javascript" / "pi"
pi_root = Path(os.environ.get("PI_MONO", default_pi)).resolve()
pi_agent = pi_root / "packages" / "agent"
vendored_module = pi_agent / "vendor" / "oche-agent-kernel.mjs"
vitest = pi_root / "node_modules" / "vitest" / "dist" / "cli.js"
source_test = root / "tests" / "pi_mono_agent_loop_acceptance.ts"
linked_test = pi_agent / "test" / ".oche-agent-loop.acceptance.test.ts"

if not (pi_root / "packages" / "agent" / "src" / "agent-loop.ts").is_file():
    raise SystemExit(f"pi-mono checkout not found at {pi_root}; set PI_MONO")
if not vitest.is_file():
    raise SystemExit(f"Vitest not installed at {vitest}; hydrate pi-mono dependencies first")
if not vendored_module.is_file():
    raise SystemExit(f"Vendored Oche kernel not found at {vendored_module}")

subprocess.run(
    ["cargo", "build", "--quiet", "--manifest-path", str(root / "Cargo.toml"), "-p", "oche"],
    check=True,
)
cli = root / "target" / "debug" / "oche"

with tempfile.TemporaryDirectory(prefix="oche-pi-mono-") as task_dir_text:
    module = Path(task_dir_text) / "pi-core.mjs"
    subprocess.run(
        [
            str(cli),
            "build-js",
            str(root / "agent" / "pi_core.oche"),
            "--single",
            "-o",
            str(module),
        ],
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert list(Path(task_dir_text).iterdir()) == [module]
    compiled = module.read_bytes()
    if compiled != vendored_module.read_bytes():
        if args.update_vendored:
            vendored_module.write_bytes(compiled)
        else:
            raise SystemExit(
                "pi-agent-core's vendored Oche kernel differs from the current compiler output; "
                "run this script with --update-vendored"
            )
    if linked_test.exists() or linked_test.is_symlink():
        raise SystemExit(f"refusing to replace existing integration test path: {linked_test}")
    linked_test.symlink_to(source_test)
    environment = {
        **os.environ,
        "OCHE_PI_CORE_URL": vendored_module.as_uri(),
        "OCHE_EXECUTABLE": str(cli),
        "OCHE_SOURCE": str(root / "agent/pi_core.oche"),
    }
    try:
        result = subprocess.run(
            ["node", str(vitest), "--run", str(linked_test.relative_to(pi_agent))],
            cwd=pi_agent,
            env=environment,
            capture_output=True,
            text=True,
            timeout=30,
            check=True,
        )
    finally:
        linked_test.unlink(missing_ok=True)

assert "1 passed" in result.stdout, result.stdout
print("Oche scheduler + pi-agent-core: model/tool/model loop passed")
