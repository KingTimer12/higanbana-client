# Código nativo do host Android em C++: hoje, a sondagem dos decodificadores de vídeo
# do aparelho (NDK MediaCodec), que escolhe o `hwdec` do mpv.
#
# Convive com as outras duas bibliotecas nativas do APK: libnsh.so (Rust: runtime do
# NSH + crate do app, compilado pelo cargo) e as do libmpv (do AAR no Maven).
cmake_minimum_required(VERSION 3.22.1)

project("higanbana_native" LANGUAGES CXX)

add_library(${CMAKE_PROJECT_NAME} SHARED
    codec/decoder_probe.cpp
    codec/hwdec_policy.cpp
    jni/codecs_bridge.cpp
    jni/jni_onload.cpp)

target_include_directories(${CMAKE_PROJECT_NAME} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
target_compile_options(${CMAKE_PROJECT_NAME} PRIVATE -Wall -Wextra -Werror -fvisibility=hidden)

target_link_libraries(${CMAKE_PROJECT_NAME}
    android
    log
    mediandk)
