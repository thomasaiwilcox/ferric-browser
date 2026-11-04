cmake_minimum_required(VERSION 3.29)
project(ferric_portal_tests LANGUAGES C CXX)
set(CMAKE_AUTOMOC ON)
set(CMAKE_CXX_STANDARD 17)
find_package(Qt6 REQUIRED COMPONENTS WaylandClient Core DBus Gui Qml Quick Widgets Test WaylandCompositor)
find_package(PkgConfig REQUIRED)
pkg_check_modules(WAYLAND_SERVER REQUIRED IMPORTED_TARGET wayland-server)
pkg_check_modules(XKBCOMMON REQUIRED IMPORTED_TARGET xkbcommon)
pkg_get_variable(WAYLAND_PROTOCOLS_DATADIR wayland-protocols pkgdatadir)
find_program(WAYLAND_SCANNER wayland-scanner REQUIRED)
find_program(DBUS_RUN_SESSION dbus-run-session REQUIRED)
find_program(DBUS_SEND dbus-send REQUIRED)
find_package(Python3 REQUIRED COMPONENTS Interpreter)
find_package(KF6WindowSystem 6.28 REQUIRED)
find_package(Qt6GuiPrivate 6.11.2 EXACT REQUIRED)
include(../../protocols/Protocols.cmake)

add_executable(portal_service service.cpp)
target_link_libraries(portal_service PRIVATE Qt6::Core Qt6::DBus)
set(TRANSPORT_SOURCES ../../src/portal_file_request.cpp ../../src/portal_file_request.h)
add_executable(portal_request_tests tst_portal_request.cpp ${TRANSPORT_SOURCES})
target_link_libraries(portal_request_tests PRIVATE Qt6::Core Qt6::DBus Qt6::Test)
add_executable(portal_activation_tests tst_portal_activation.cpp ${TRANSPORT_SOURCES})
target_link_libraries(portal_activation_tests PRIVATE Qt6::Core Qt6::DBus Qt6::Test)
add_executable(portal_file_chooser_tests tst_portal_file_chooser.cpp ${TRANSPORT_SOURCES}
    ../../src/native_file_chooser.cpp ../../src/native_file_chooser.h
    ../../src/local_file_url.cpp ../../src/local_file_url.h
    ../../src/gui_application.cpp ../../src/gui_application.h)
target_link_libraries(portal_file_chooser_tests PRIVATE KF6::WindowSystem Qt6::DBus Qt6::Test Qt6::Quick Qt6::Widgets PkgConfig::FERRIC_WAYLAND_CLIENT)
ferric_native_wayland(portal_file_chooser_tests)

add_executable(portal_compositor compositor.cpp $<TARGET_OBJECTS:ferric_wayland_protocols>)
add_dependencies(portal_compositor ferric_wayland_protocols)
target_include_directories(portal_compositor PRIVATE "${FERRIC_PROTOCOL_OUTPUT}")
target_link_libraries(portal_compositor PRIVATE Qt6::Gui Qt6::DBus Qt6::WaylandCompositor PkgConfig::WAYLAND_SERVER PkgConfig::XKBCOMMON)
enable_testing()
add_executable(portal_native_tests tst_native_wayland.cpp)
ferric_native_wayland(portal_native_tests)
target_link_libraries(portal_native_tests PRIVATE Qt6::DBus Qt6::Widgets Qt6::Test)
add_executable(portal_qt_control_tests tst_qt_lifecycle_control.cpp)
target_link_libraries(portal_qt_control_tests PRIVATE Qt6::Gui Qt6::Widgets Qt6::Test)
add_test(NAME native_qt_lifecycle_control COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/../application/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/run.py" "${CMAKE_CURRENT_BINARY_DIR}" qt_control)
set_tests_properties(native_qt_lifecycle_control PROPERTIES TIMEOUT 90)
foreach(variant IN ITEMS full no_tags no_exporter neither)
    set(missing)
    if(variant STREQUAL "no_tags" OR variant STREQUAL "neither")
        list(APPEND missing FERRIC_FIXTURE_NO_TAGS=1)
    endif()
    if(variant STREQUAL "no_exporter" OR variant STREQUAL "neither")
        list(APPEND missing FERRIC_FIXTURE_NO_EXPORTER=1)
    endif()
    add_test(NAME native_wayland_${variant} COMMAND "${DBUS_RUN_SESSION}"
        --config-file=${CMAKE_CURRENT_SOURCE_DIR}/../application/notification-test-bus.conf --
        ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated ${missing} "${Python3_EXECUTABLE}"
        "${CMAKE_CURRENT_SOURCE_DIR}/run.py" "${CMAKE_CURRENT_BINARY_DIR}" native)
    set_tests_properties(native_wayland_${variant} PROPERTIES TIMEOUT 90)
endforeach()
set(PORTAL_TESTS request file_chooser)
if(DEFINED FERRIC_ENGINE_LIBRARY)
    find_package(Qt6 REQUIRED COMPONENTS QuickControls2 WebEngineQuick Network)
    find_package(SQLite3 REQUIRED)
    if(NOT TARGET SQLite3::SQLite3)
        add_library(SQLite3::SQLite3 ALIAS SQLite::SQLite3)
    endif()
    add_executable(portal_browser_tests tst_browser_file_choosers.cpp)
    find_package(Qt6 REQUIRED COMPONENTS WebChannel QuickWidgets WebEngineWidgets)
    target_link_libraries(portal_browser_tests PRIVATE "${FERRIC_ENGINE_LIBRARY}" Qt6::WaylandClient Qt6::WebChannel Qt6::QuickWidgets Qt6::WebEngineWidgets PkgConfig::FERRIC_WAYLAND_CLIENT
        Qt6::Quick Qt6::QuickControls2 Qt6::WebEngineQuick Qt6::Widgets Qt6::DBus Qt6::Network Qt6::Test
        SQLite3::SQLite3 ${CMAKE_DL_LIBS} pthread)
    list(APPEND PORTAL_TESTS browser)
    add_test(NAME portal_download_history COMMAND "${Python3_EXECUTABLE}"
        "${CMAKE_CURRENT_SOURCE_DIR}/run_history.py" "${CMAKE_CURRENT_BINARY_DIR}" "${DBUS_RUN_SESSION}")
    set_tests_properties(portal_download_history PROPERTIES TIMEOUT 380)
endif()
add_test(NAME portal_activation COMMAND "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/run_activation.py" "${CMAKE_CURRENT_BINARY_DIR}" "${DBUS_RUN_SESSION}")
set_tests_properties(portal_activation PROPERTIES TIMEOUT 75)
foreach(test IN LISTS PORTAL_TESTS)
    add_test(NAME portal_${test} COMMAND "${DBUS_RUN_SESSION}"
        --config-file=${CMAKE_CURRENT_SOURCE_DIR}/../application/notification-test-bus.conf --
        ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
        "${CMAKE_CURRENT_SOURCE_DIR}/run.py" "${CMAKE_CURRENT_BINARY_DIR}" "${test}")
    set_tests_properties(portal_${test} PROPERTIES TIMEOUT 180)
endforeach()
