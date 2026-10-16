import 'dart:ffi' as ffi;
import 'dart:io' show Platform;

ffi.DynamicLibrary _openSystemLibrary() {
  if (Platform.isLinux) {
    return ffi.DynamicLibrary.open('libc.so.6');
  }
  if (Platform.isMacOS) {
    return ffi.DynamicLibrary.open('/usr/lib/libSystem.B.dylib');
  }
  if (Platform.isWindows) {
    return ffi.DynamicLibrary.open('msvcrt.dll');
  }
  throw UnsupportedError('system FFI probe is not supported on this platform');
}

typedef _GetPidNative = ffi.Int32 Function();
typedef _GetPidDart = int Function();

final ffi.DynamicLibrary _systemLibrary = _openSystemLibrary();
final _GetPidDart _getPid = _systemLibrary
    .lookup<ffi.NativeFunction<_GetPidNative>>(
      Platform.isWindows ? '_getpid' : 'getpid',
    )
    .asFunction();

int systemProcessId() => _getPid();
