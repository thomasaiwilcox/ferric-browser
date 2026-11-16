cmake_minimum_required(VERSION 3.20)
project(ferric_qml_tests LANGUAGES C CXX)
set(CMAKE_AUTOMOC ON)
set(CMAKE_CXX_STANDARD 17)
find_package(Qt6GuiPrivate 6.11.2 EXACT REQUIRED)
find_package(Qt6 6.11.2 EXACT REQUIRED COMPONENTS WaylandClient)
include(../../protocols/Protocols.cmake)
find_package(Qt6 REQUIRED COMPONENTS QuickTest WebEngineQuick WebEngineCorePrivate CorePrivate Qml Widgets DBus)
add_executable(qml_tests main.cpp ../../src/browser_key_router.cpp ../../src/browser_key_router.h ../../src/script_runner.cpp ../../src/script_runner.h ../../src/settings_model.cpp ../../src/settings_model.h ../../src/native_file_chooser.cpp ../../src/native_file_chooser.h ../../src/portal_file_request.cpp ../../src/portal_file_request.h ../../src/gui_application.cpp ../../src/gui_application.h ../../src/local_file_url.cpp ../../src/local_file_url.h ../../src/download_source.cpp ../../src/download_source.h)
target_sources(qml_tests PRIVATE ../../src/hint_frame_broker.cpp ../../src/hint_frame_broker.h)
target_link_libraries(qml_tests PRIVATE PkgConfig::FERRIC_WAYLAND_CLIENT Qt6::DBus Qt6::QuickTest Qt6::WebEngineQuick Qt6::Qml Qt6::Widgets)
target_link_libraries(qml_tests PRIVATE Qt6::WebEngineCorePrivate Qt6::CorePrivate)
target_include_directories(qml_tests PRIVATE ../../src/qt_compat)
set_source_files_properties(../../qml/scripts/PageInteractionScripts.js PROPERTIES QT_RESOURCE_ALIAS "PageInteractionScripts.js")
qt_add_resources(qml_tests hint_geometry_scripts
    PREFIX "/qt/qml/io/github/ferricbrowser/qml/scripts"
    FILES ../../qml/scripts/PageInteractionScripts.js)

ferric_native_wayland(qml_tests)
