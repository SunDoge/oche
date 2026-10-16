"""Compile fixtures and run the Rust binding in an ordinary Dart JIT VM."""
import json
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parent.parent
subprocess.run(['cargo', 'build', '--manifest-path', str(root/'crates/dart-vm/Cargo.toml'),
                '--release', '--example', 'kernel_probe'], check=True)
sdk_bin = Path(subprocess.check_output(['mise', 'which', 'dart'], text=True).strip()).resolve().parent
library = root/'target/release/examples/libkernel_probe.so'
if not library.exists():
    raise RuntimeError('The current process resolver and runner target Linux')
with tempfile.TemporaryDirectory(prefix='dart-rust-binding-') as task_dir:
    directory = Path(task_dir)
    count = 60
    for version in range(1, count+1):
        source = directory/f'tool_v{version}.dart'
        source.write_text(f"@pragma('vm:entry-point')\nconst version = {version};\n"
            "@pragma('vm:entry-point')\nvoid register(Map<String, Function> tools) {\n"
            f"  final add = (int a, int b) => a + b + {version-1};\n"
            f"  tools['add'] = add; tools['add_v{version}'] = add;\n"
            "}\nvoid main() {}\n")
        result = subprocess.run([str(sdk_bin/'dart'), 'compile', 'kernel', str(source),
            '-o', str(source.with_suffix('.dill')), '--no-link-platform', '--no-embed-sources'],
            capture_output=True, text=True, timeout=30)
        if result.returncode: raise RuntimeError(result.stdout + result.stderr)
    host = directory/'host.dill'
    result = subprocess.run([str(sdk_bin/'dart'), 'compile', 'kernel',
        str(root/'tests/rust_ffi_host.dart'), '-o', str(host)], capture_output=True, text=True, timeout=30)
    if result.returncode: raise RuntimeError(result.stdout + result.stderr)
    result = subprocess.run([str(sdk_bin/'dartvm'), str(host), str(library), str(directory), str(count)],
        capture_output=True, text=True, timeout=30)
    if result.returncode: raise RuntimeError(f'exit {result.returncode}: {result.stdout}\n{result.stderr}')
    report = json.loads(result.stdout)
    report['rust_library_bytes'] = library.stat().st_size
    (root/'tests/probe-results.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps(report, indent=2))
