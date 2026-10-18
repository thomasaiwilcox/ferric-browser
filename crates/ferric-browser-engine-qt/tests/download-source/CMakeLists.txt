cmake_minimum_required(VERSION 3.20)
project(ferric_download_source_tests LANGUAGES CXX)
set(CMAKE_AUTOMOC ON)
set(CMAKE_CXX_STANDARD 17)
find_package(Qt6 REQUIRED COMPONENTS Quick Test Network WebEngineQuick WebEngineCorePrivate CorePrivate)
add_executable(download_source_tests
    tst_download_source.cpp ../../src/download_source.h ../../src/download_source.cpp)
target_include_directories(download_source_tests PRIVATE ../../src/qt_compat)
target_link_libraries(download_source_tests PRIVATE Qt6::Quick Qt6::Test Qt6::Network
    Qt6::WebEngineQuick Qt6::WebEngineCorePrivate Qt6::CorePrivate)
enable_testing()
add_test(NAME download_source COMMAND download_source_tests)
set_tests_properties(download_source PROPERTIES TIMEOUT 60
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QPA_PLATFORMTHEME=;QT_QUICK_BACKEND=software;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
