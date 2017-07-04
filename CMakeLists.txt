PROJECT(miniz)
cmake_minimum_required(VERSION 2.8)
set(CMAKE_BUILD_TYPE Release)
set(CMAKE_CONFIGURATION_TYPES Release Debug)

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_SOURCE_DIR}/bin)

set(miniz_SOURCE miniz.c miniz_zip.c miniz_tinfl.c miniz_tdef.c)

add_library(miniz ${miniz_SOURCE})

include(ExternalProject)

ExternalProject_Add(
    miniz_oxide
    DOWNLOAD_COMMAND ""
    CONFIGURE_COMMAND ""
    BUILD_COMMAND cargo build COMMAND cargo build --release
    BINARY_DIR "${CMAKE_SOURCE_DIR}"
    INSTALL_COMMAND ""
    LOG_BUILD ON)

add_dependencies(miniz miniz_oxide)

target_link_libraries(miniz
    debug "${CMAKE_SOURCE_DIR}/common-rust/target/debug/librust_example.a"
    optimized "${CMAKE_SOURCE_DIR}/common-rust/target/release/librust_example.a"
    dl rt pthread gcc_s c m rt pthread util)
