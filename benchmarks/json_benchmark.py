"""Benchmark generated typed JSON codecs against conventional Dart maps."""

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
generated = subprocess.run(
    [str(cli), "emit-dart", str(example)],
    capture_output=True,
    text=True,
    timeout=30,
    check=True,
).stdout
generated = generated.replace(
    "Future<void> main(List<String> arguments) async {",
    "Future<void> ocheCliMain(List<String> arguments) async {",
    1,
)

harness = r'''
final class DartUser {
  const DartUser(this.name, this.age, this.active);
  final String name;
  final int age;
  final bool active;
}

const dartUser = DartUser('Ada Lovelace', 37, true);
const ocheUser = OcheStruct0(f0: 'Ada Lovelace', f1: 37, f2: true);
const encodedUser = '{"name":"Ada Lovelace","age":37,"active":true}';
var benchmarkSink = 0;

void dartEncode() {
  final encoded = jsonEncode(<String, Object?>{
    'name': dartUser.name,
    'age': dartUser.age,
    'active': dartUser.active,
  });
  benchmarkSink ^= encoded.length;
}

void ocheEncode() {
  final encoded = ocheJsonTo0(ocheUser);
  benchmarkSink ^= encoded.length;
}

void dartDecode() {
  final value = jsonDecode(encodedUser) as Map<String, Object?>;
  final decoded = DartUser(
    value['name'] as String,
    value['age'] as int,
    value['active'] as bool,
  );
  benchmarkSink ^= decoded.age;
}

void ocheDecode() {
  final decoded = ocheJsonFrom0(encodedUser);
  switch (decoded) {
    case OcheOk(value: final value):
      benchmarkSink ^= value.f1;
    case OcheErr(error: final error):
      throw StateError(error);
  }
}

double measure(String label, int iterations, void Function() operation) {
  final warmup = iterations ~/ 5;
  for (var i = 0; i < warmup; i++) operation();
  final samples = <int>[];
  for (var sample = 0; sample < 7; sample++) {
    final watch = Stopwatch()..start();
    for (var i = 0; i < iterations; i++) operation();
    watch.stop();
    samples.add(watch.elapsedMicroseconds);
  }
  samples.sort();
  final nanoseconds = samples[samples.length ~/ 2] * 1000 / iterations;
  final operations = 1000000000 / nanoseconds;
  print('$label: ${nanoseconds.toStringAsFixed(1)} ns/op, '
      '${operations.toStringAsFixed(0)} ops/s');
  return nanoseconds;
}

void main(List<String> arguments) {
  final iterations = arguments.isEmpty ? 200000 : int.parse(arguments[0]);
  final dartEncodeTime = measure('Dart Map encode', iterations, dartEncode);
  final ocheEncodeTime = measure('Oche typed encode', iterations, ocheEncode);
  final dartDecodeTime = measure('Dart Map decode', iterations, dartDecode);
  final ocheDecodeTime = measure('Oche typed decode', iterations, ocheDecode);
  print('encode speedup: ${(dartEncodeTime / ocheEncodeTime).toStringAsFixed(2)}x');
  print('decode speedup: ${(dartDecodeTime / ocheDecodeTime).toStringAsFixed(2)}x');
  print('sink: $benchmarkSink');
}
'''

with tempfile.TemporaryDirectory(prefix="oche-json-benchmark-") as directory_text:
    directory = Path(directory_text)
    source = directory / "json_benchmark.dart"
    source.write_text(generated + "\n" + harness)

    print("JIT (warmed):", flush=True)
    subprocess.run(["dart", "run", str(source), "200000"], check=True, timeout=120)

    executable = directory / "json_benchmark"
    subprocess.run(
        ["dart", "compile", "exe", str(source), "-o", str(executable)],
        check=True,
        capture_output=True,
        text=True,
        timeout=120,
    )
    print("AOT:", flush=True)
    subprocess.run([str(executable), "500000"], check=True, timeout=120)
