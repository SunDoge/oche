import 'dart:convert';
import 'dart:ffi';

@pragma('vm:entry-point')
final tools = <String, Function>{};

void _registerTool(String name, Function implementation) {
  if (tools.containsKey(name)) {
    throw StateError('tool already registered: $name');
  }
  tools[name] = implementation;
}

@pragma('vm:entry-point')
final void Function(String, Function) registerTool = _registerTool;

@pragma('vm:entry-point')
Function registeredTool(String name) => tools[name] as Function;

@pragma('vm:entry-point')
int callTool(int a, int b) => (tools['add'] as int Function(int, int))(a, b);

late final DynamicLibrary rustLibrary;

void main(List<String> arguments) {
  rustLibrary = DynamicLibrary.open(arguments[0]);
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
  Pointer<Uint8> allocatePath(String value) {
    final bytes = utf8.encode(value);
    final path = malloc(bytes.length + 1).cast<Uint8>();
    if (path == nullptr) throw StateError('malloc failed');
    path.asTypedList(bytes.length + 1).setAll(0, [...bytes, 0]);
    return path;
  }

  final toolPath = allocatePath(arguments[1]);
  final agentPath = allocatePath(arguments[2]);
  final probe = rustLibrary
      .lookupFunction<
        Pointer<Uint8> Function(Pointer<Uint8>, Pointer<Uint8>),
        Pointer<Uint8> Function(Pointer<Uint8>, Pointer<Uint8>)
      >('dart_oche_module_probe');
  final release = rustLibrary
      .lookupFunction<
        Void Function(Pointer<Uint8>),
        void Function(Pointer<Uint8>)
      >('dart_oche_module_probe_free');
  Pointer<Uint8> report = nullptr;
  try {
    report = probe(toolPath, agentPath);
    if (report == nullptr) throw StateError('Rust returned a null report');
    var length = 0;
    while (length < 1024 * 1024 && report[length] != 0) {
      length++;
    }
    if (length == 1024 * 1024) throw StateError('Report exceeds limit');
    final text = utf8.decode(report.asTypedList(length));
    if (text.startsWith('ERROR:')) throw StateError(text);
    print(text);
  } finally {
    if (report != nullptr) release(report);
    free(toolPath.cast<Void>());
    free(agentPath.cast<Void>());
  }
}
