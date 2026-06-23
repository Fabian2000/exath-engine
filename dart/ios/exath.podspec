Pod::Spec.new do |s|
  s.name             = 'exath'
  s.version          = '1.0.2'
  s.summary          = 'exath engine (prebuilt native library)'
  s.homepage         = 'https://github.com/Fabian2000/exath-engine'
  s.license          = { :type => 'MIT OR Apache-2.0' }
  s.author           = { 'Fabian2000' => 'fs.21012000@gmail.com' }
  s.source           = { :path => '.' }
  s.source_files     = 'Classes/**/*'
  s.vendored_frameworks = 'exath_engine_ffi.xcframework'
  s.dependency 'Flutter'
  s.platform         = :ios, '12.0'
  s.pod_target_xcconfig = { 'DEFINES_MODULE' => 'YES' }
end
