Pod::Spec.new do |s|
  s.name             = 'exath'
  s.version          = '1.0.1'
  s.summary          = 'exath engine (prebuilt native library)'
  s.homepage         = 'https://github.com/Fabian2000/exath-engine'
  s.license          = { :type => 'MIT OR Apache-2.0' }
  s.author           = { 'Fabian2000' => 'fs.21012000@gmail.com' }
  s.source           = { :path => '.' }
  s.source_files     = 'Classes/**/*'
  s.vendored_libraries = 'Frameworks/libexath_engine_ffi.dylib'
  s.dependency 'FlutterMacOS'
  s.platform         = :osx, '10.14'
  s.pod_target_xcconfig = { 'DEFINES_MODULE' => 'YES' }
end
