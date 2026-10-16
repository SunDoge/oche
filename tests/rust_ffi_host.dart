import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

@pragma('vm:entry-point')
final tools = <String, Function>{};
@pragma('vm:entry-point')
int callTool(int a, int b) => (tools['add'] as int Function(int, int))(a, b);
@pragma('vm:entry-point')
int callVersion(int version, int a, int b) =>
    (tools['add_v$version'] as int Function(int, int))(a, b);

// Keep Rust's finalizer code loaded until the VM exits.
late final DynamicLibrary rustLibrary;

void main(List<String> args) {
  rustLibrary = DynamicLibrary.open(args[0]);
  final count = int.parse(args[2]);
  final process = DynamicLibrary.process();
  final malloc = process
      .lookupFunction<
        Pointer<Void> Function(IntPtr),
        Pointer<Void> Function(int)
      >('malloc');
  final free = process
      .lookupFunction<
        Void Function(Pointer<Void>),
        void Function(Pointer<Void>)
      >('free');
  final allocations = <Pointer<Void>>[];
  Pointer<Uint8> cString(String value) {
    final bytes = utf8.encode(value);
    final memory = malloc(bytes.length + 1);
    if (memory == nullptr) throw StateError('malloc failed');
    allocations.add(memory);
    final result = memory.cast<Uint8>();
    result.asTypedList(bytes.length + 1).setAll(0, [...bytes, 0]);
    return result;
  }

  final rawPaths = malloc(count * sizeOf<Pointer<Uint8>>());
  if (rawPaths == nullptr) throw StateError('malloc failed');
  allocations.add(rawPaths);
  final paths = rawPaths.cast<Pointer<Uint8>>();
  for (var i = 0; i < count; i++) {
    paths[i] = cString('${args[1]}/tool_v${i + 1}.dill');
  }
  final probe = rustLibrary
      .lookupFunction<
        Pointer<Uint8> Function(Pointer<Pointer<Uint8>>, IntPtr),
        Pointer<Uint8> Function(Pointer<Pointer<Uint8>>, int)
      >('dart_kernel_probe');
  final release = rustLibrary
      .lookupFunction<
        Void Function(Pointer<Uint8>),
        void Function(Pointer<Uint8>)
      >('dart_kernel_probe_free');
  Pointer<Uint8> report = nullptr;
  final initialRss = ProcessInfo.currentRss;
  try {
    // Non-leaf FFI is required: Rust re-enters Dart via the embedding API.
    report = probe(paths, count);
    if (report == nullptr) throw StateError('Rust returned a null report');
    var length = 0;
    while (length < 1024 * 1024 && report[length] != 0) {
      length++;
    }
    if (length == 1024 * 1024) throw StateError('Report exceeds limit');
    final text = utf8.decode(report.asTypedList(length));
    if (text.startsWith('ERROR:')) throw StateError(text);
    final result = jsonDecode(text) as Map<String, Object?>;
    result.addAll({
      'dart': Platform.version,
      'initial_rss_mib': initialRss / 1024 / 1024,
      'final_rss_mib': ProcessInfo.currentRss / 1024 / 1024,
      'rss_delta_mib': (ProcessInfo.currentRss - initialRss) / 1024 / 1024,
    });
    print(jsonEncode(result));
  } finally {
    if (report != nullptr) release(report);
    for (final memory in allocations) {
      free(memory);
    }
  }
}
