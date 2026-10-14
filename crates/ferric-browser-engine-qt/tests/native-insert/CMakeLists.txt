cmake_minimum_required(VERSION 3.20)
project(ferric_native_insert LANGUAGES CXX)
set(CMAKE_AUTOMOC ON)
set(CMAKE_CXX_STANDARD 17)
find_package(Qt6 REQUIRED COMPONENTS Quick WebEngineQuick Test)
add_executable(native_insert tst_native_insert.cpp ../../src/browser_key_router.cpp ../../src/browser_page_pointer_adapter.cpp)
target_link_libraries(native_insert PRIVATE Qt6::Quick Qt6::WebEngineQuick Qt6::Test)
target_compile_definitions(native_insert PRIVATE
    FERRIC_SCRIPT_PATH="${CMAKE_CURRENT_SOURCE_DIR}/../../qml/scripts/BrowserScripts.js"
    FERRIC_DRAG_FIXTURE_PATH="${CMAKE_CURRENT_SOURCE_DIR}/../fixtures/drag-probe.html")
enable_testing()
add_test(NAME native_insert COMMAND native_insert)
set_tests_properties(native_insert PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
