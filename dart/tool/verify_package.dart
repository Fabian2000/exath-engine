// Builds the package exactly as `dart pub publish` would (via --dry-run) and
// asserts that the bundled native libraries and the WASM assets are actually
// present. Catches the class of bug where a declared asset silently drops out
// of the published archive.
//
// Run before publishing:  dart run tool/verify_package.dart
import 'dart:io';

const _required = [
  'assets/wasm/exath_engine_wasm.js',
  'assets/wasm/exath_engine_wasm_bg.wasm',
  'jniLibs/arm64-v8a/libexath_engine_ffi.so',
  'jniLibs/armeabi-v7a/libexath_engine_ffi.so',
  'jniLibs/x86_64/libexath_engine_ffi.so',
  'exath_engine_ffi.xcframework',
  'macos/Frameworks/libexath_engine_ffi.dylib',
  'linux/libexath_engine_ffi.so',
  'windows/exath_engine_ffi.dll',
];

Future<void> main() async {
  stdout.writeln('Running `dart pub publish --dry-run` to inspect package...');
  final r = await Process.run('dart', ['pub', 'publish', '--dry-run']);
  final out = '${r.stdout}\n${r.stderr}';

  final missing = _required.where((f) => !out.contains(f.split('/').last));
  if (missing.isNotEmpty) {
    stderr.writeln('\nPACKAGE VERIFICATION FAILED. Missing from the archive:');
    for (final m in missing) {
      stderr.writeln('  - $m');
    }
    stderr.writeln('\nRun `dart run tool/fetch_native.dart` first, and check '
        'for stray .gitignore files inside bundled asset folders.');
    exit(1);
  }
  stdout.writeln('OK: all native libraries and WASM assets are in the package.');
}
