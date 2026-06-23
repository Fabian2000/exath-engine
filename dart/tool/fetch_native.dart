// Fetches the prebuilt native libraries (and the WASM bundle) from the
// exath-engine GitHub release and lays them out in the plugin's platform
// folders. Run before building the example or publishing:
//
//   dart run tool/fetch_native.dart
//
// Used by CI (the binaries are gitignored, not committed).
import 'dart:io';
import 'dart:typed_data';

import 'package:archive/archive_io.dart';

const _tag = 'v1.0.0';
const _base =
    'https://github.com/Fabian2000/exath-engine/releases/download/$_tag';

Future<List<int>> _download(String name) async {
  final client = HttpClient();
  try {
    final req = await client.getUrl(Uri.parse('$_base/$name'));
    final resp = await req.close();
    if (resp.statusCode != 200) {
      throw Exception('download $name failed: HTTP ${resp.statusCode}');
    }
    final out = BytesBuilder();
    await for (final chunk in resp) {
      out.add(chunk);
    }
    return out.takeBytes();
  } finally {
    client.close();
  }
}

Future<void> _file(String name, String dest) async {
  File(dest).parent.createSync(recursive: true);
  File(dest).writeAsBytesSync(await _download(name));
  stdout.writeln('  $dest');
}

Future<void> _targz(String name, String destDir) async {
  final bytes = await _download(name);
  final archive = TarDecoder().decodeBytes(GZipDecoder().decodeBytes(bytes));
  extractArchiveToDisk(archive, destDir);
  stdout.writeln('  $destDir/ (from $name)');
}

Future<void> _zip(String name, String destDir) async {
  final archive = ZipDecoder().decodeBytes(await _download(name));
  extractArchiveToDisk(archive, destDir);
  stdout.writeln('  $destDir/ (from $name)');
}

Future<void> main() async {
  stdout.writeln('Fetching prebuilt native libraries ($_tag)...');
  await _targz('exath-engine-ffi-android-jniLibs.tar.gz', 'android/src/main');
  await _zip('exath-engine-ffi-ios-xcframework.zip', 'ios');
  await _file('libexath_engine_ffi-macos-arm64.dylib',
      'macos/Frameworks/libexath_engine_ffi.dylib');
  await _file('libexath_engine_ffi-linux-x64.so', 'linux/libexath_engine_ffi.so');
  await _file('exath_engine_ffi-windows-x64.dll', 'windows/exath_engine_ffi.dll');
  await _targz('exath-engine-wasm.tar.gz', 'assets/wasm');
  stdout.writeln('Done.');
}
