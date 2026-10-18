cmake_minimum_required(VERSION 3.29)
project(ferric_portal_tests LANGUAGES C CXX)
set(CMAKE_AUTOMOC ON)
set(CMAKE_CXX_STANDARD 17)
find_package(Qt6 REQUIRED COMPONENTS Core DBus Gui Qml Quick Widgets Test WaylandCompositor)
find_package(KF6WindowSystem 6.28 REQUIRED)
find_package(PkgConfig REQUIRED)
pkg_check_modules(WAYLAND_SERVER REQUIRED IMPORTED_TARGET wayland-server)
pkg_get_variable(WAYLAND_PROTOCOLS_DATADIR wayland-protocols pkgdatadir)
find_program(WAYLAND_SCANNER wayland-scanner REQUIRED)
find_program(DBUS_RUN_SESSION dbus-run-session REQUIRED)
find_package(Python3 REQUIRED COMPONENTS Interpreter)

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
target_link_libraries(portal_file_chooser_tests PRIVATE Qt6::DBus Qt6::Test Qt6::Quick Qt6::Widgets KF6::WindowSystem)

set(FOREIGN_PROTOCOL "${WAYLAND_PROTOCOLS_DATADIR}/unstable/xdg-foreign/xdg-foreign-unstable-v2.xml")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/foreign-server.h"
    COMMAND "${WAYLAND_SCANNER}" server-header "${FOREIGN_PROTOCOL}" "${CMAKE_CURRENT_BINARY_DIR}/foreign-server.h"
    DEPENDS "${FOREIGN_PROTOCOL}")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/foreign-protocol.c"
    COMMAND "${WAYLAND_SCANNER}" private-code "${FOREIGN_PROTOCOL}" "${CMAKE_CURRENT_BINARY_DIR}/foreign-protocol.c"
    DEPENDS "${FOREIGN_PROTOCOL}")
add_executable(portal_compositor compositor.cpp "${CMAKE_CURRENT_BINARY_DIR}/foreign-server.h" "${CMAKE_CURRENT_BINARY_DIR}/foreign-protocol.c")
target_include_directories(portal_compositor PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
target_link_libraries(portal_compositor PRIVATE Qt6::Gui Qt6::DBus Qt6::WaylandCompositor PkgConfig::WAYLAND_SERVER)
enable_testing()
set(PORTAL_TESTS request file_chooser)
if(DEFINED FERRIC_ENGINE_LIBRARY)
    find_package(Qt6 REQUIRED COMPONENTS QuickControls2 WebEngineQuick Network)
    find_package(SQLite3 REQUIRED)
    if(NOT TARGET SQLite3::SQLite3)
        add_library(SQLite3::SQLite3 ALIAS SQLite::SQLite3)
    endif()
    add_executable(portal_browser_tests tst_browser_file_choosers.cpp)
    target_link_libraries(portal_browser_tests PRIVATE "${FERRIC_ENGINE_LIBRARY}" KF6::WindowSystem
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
