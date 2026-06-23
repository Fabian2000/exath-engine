cmake_minimum_required(VERSION 3.10)
set(PROJECT_NAME "exath")
project(${PROJECT_NAME} LANGUAGES CXX)

# Prebuilt shared library, bundled with the Flutter app (no compilation).
set(exath_bundled_libraries
  "${CMAKE_CURRENT_SOURCE_DIR}/libexath_engine_ffi.so"
  PARENT_SCOPE
)
