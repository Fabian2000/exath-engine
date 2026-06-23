import 'dart:ffi';
import 'dart:io';

/// Release tag the prebuilt artifacts are fetched from. Matches the engine
/// version; bump together with `pubspec.yaml`.
const exathVersion = '1.0.0';

const _releaseBase =
    'https://github.com/Fabian2000/exath-engine/releases/download';

/// The prebuilt artifact name for the current platform/architecture, matching
/// the names produced by `.github/workflows/release.yml`. Returns null on an
/// unsupported platform (e.g. mobile, which bundles the library instead).
String? nativeAssetName() {
  switch (Abi.current()) {
    case Abi.linuxX64:
      return 'libexath_engine_ffi-linux-x64.so';
    case Abi.linuxArm64:
      return 'libexath_engine_ffi-linux-arm64.so';
    case Abi.macosX64:
      return 'libexath_engine_ffi-macos-x64.dylib';
    case Abi.macosArm64:
      return 'libexath_engine_ffi-macos-arm64.dylib';
    case Abi.windowsX64:
      return 'exath_engine_ffi-windows-x64.dll';
    default:
      return null;
  }
}

/// Per-user cache directory for the downloaded library (version-scoped).
String cacheDir() {
  final sep = Platform.pathSeparator;
  final base = Platform.isWindows
      ? (Platform.environment['LOCALAPPDATA'] ?? Directory.systemTemp.path)
      : (Platform.environment['HOME'] ?? Directory.systemTemp.path);
  return '$base$sep.exath$sep$exathVersion';
}

/// Full path where the downloaded library is (or would be) cached, or null if
/// the platform has no downloadable prebuilt artifact.
String? cachedLibPath() {
  final name = nativeAssetName();
  if (name == null) return null;
  return '${cacheDir()}${Platform.pathSeparator}$name';
}

/// Download the prebuilt native library for the current platform from the
/// matching GitHub release into [cacheDir]. Returns the local path. No-op if
/// already cached. Throws on unsupported platform or a failed download.
Future<String> downloadNativeLibrary() async {
  final name = nativeAssetName();
  if (name == null) {
    throw UnsupportedError(
        'exath: no prebuilt library for ${Abi.current()} (mobile bundles it)');
  }
  final dest = cachedLibPath()!;
  if (File(dest).existsSync()) return dest;

  Directory(cacheDir()).createSync(recursive: true);
  final url = '$_releaseBase/v$exathVersion/$name';
  final client = HttpClient();
  try {
    final request = await client.getUrl(Uri.parse(url));
    final response = await request.close();
    if (response.statusCode != 200) {
      throw Exception('exath: download failed ($url): HTTP ${response.statusCode}');
    }
    final sink = File(dest).openWrite();
    await response.pipe(sink);
    return dest;
  } finally {
    client.close();
  }
}
