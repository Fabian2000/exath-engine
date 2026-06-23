cmake_minimum_required(VERSION 3.14)
set(PROJECT_NAME "exath")
project(${PROJECT_NAME} LANGUAGES CXX)

set(exath_bundled_libraries
  "${CMAKE_CURRENT_SOURCE_DIR}/exath_engine_ffi.dll"
  PARENT_SCOPE
)
