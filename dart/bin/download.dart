// Downloads the prebuilt exath native library for the current platform from the
// matching GitHub release, so no Rust toolchain is needed.
//
//   dart run exath:download
//
import 'package:exath/src/native_install.dart';

Future<void> main() async {
  final name = nativeAssetName();
  if (name == null) {
    print('exath: this platform has no downloadable prebuilt library '
        '(mobile targets bundle it via the Flutter plugin).');
    return;
  }
  print('exath: downloading $name (v$exathVersion)...');
  final path = await downloadNativeLibrary();
  print('exath: installed at $path');
}
