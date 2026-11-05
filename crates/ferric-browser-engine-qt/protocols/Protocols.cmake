include_guard(GLOBAL)
find_package(Python3 REQUIRED COMPONENTS Interpreter)
find_package(PkgConfig REQUIRED)
pkg_check_modules(FERRIC_WAYLAND_CLIENT REQUIRED IMPORTED_TARGET wayland-client>=1.20)
find_program(FERRIC_WAYLAND_SCANNER wayland-scanner REQUIRED)
set(FERRIC_PROTOCOL_ROOT "${CMAKE_CURRENT_LIST_DIR}")
set(FERRIC_PROTOCOL_OUTPUT "${CMAKE_BINARY_DIR}/ferric-protocols")
set(FERRIC_PROTOCOL_SOURCES)
set(FERRIC_PROTOCOL_HEADERS)
foreach(name IN ITEMS xdg-foreign-unstable-v2 xdg-toplevel-tag-v1 xdg-shell)
    list(APPEND FERRIC_PROTOCOL_SOURCES "${FERRIC_PROTOCOL_OUTPUT}/${name}-protocol.c")
    list(APPEND FERRIC_PROTOCOL_HEADERS
        "${FERRIC_PROTOCOL_OUTPUT}/${name}-client.h"
        "${FERRIC_PROTOCOL_OUTPUT}/${name}-server.h")
endforeach()
add_custom_command(OUTPUT ${FERRIC_PROTOCOL_SOURCES} ${FERRIC_PROTOCOL_HEADERS}
    COMMAND "${Python3_EXECUTABLE}" "${FERRIC_PROTOCOL_ROOT}/generate.py"
        --scanner "${FERRIC_WAYLAND_SCANNER}" --output "${FERRIC_PROTOCOL_OUTPUT}"
    DEPENDS "${FERRIC_PROTOCOL_ROOT}/generate.py" "${FERRIC_PROTOCOL_ROOT}/provenance.json"
        "${FERRIC_PROTOCOL_ROOT}/xdg-foreign-unstable-v2.xml"
        "${FERRIC_PROTOCOL_ROOT}/xdg-toplevel-tag-v1.xml"
        "${FERRIC_PROTOCOL_ROOT}/xdg-shell.xml"
    VERBATIM)
add_library(ferric_wayland_protocols OBJECT ${FERRIC_PROTOCOL_SOURCES} ${FERRIC_PROTOCOL_HEADERS})
target_link_libraries(ferric_wayland_protocols PRIVATE PkgConfig::FERRIC_WAYLAND_CLIENT)
set_target_properties(ferric_wayland_protocols PROPERTIES AUTOMOC OFF POSITION_INDEPENDENT_CODE ON)

# Call only for executables that compile the bridge directly. Engine-linked
# executables get the protocol implementation from the engine archive.
function(ferric_native_wayland target)
    target_sources(${target} PRIVATE
        "${FERRIC_PROTOCOL_ROOT}/../src/native_wayland.cpp"
        "${FERRIC_PROTOCOL_ROOT}/../src/native_wayland.h"
        $<TARGET_OBJECTS:ferric_wayland_protocols>)
    add_dependencies(${target} ferric_wayland_protocols)
    target_include_directories(${target} PRIVATE "${FERRIC_PROTOCOL_OUTPUT}")
    target_link_libraries(${target} PRIVATE Qt6::GuiPrivate Qt6::WaylandClient
        PkgConfig::FERRIC_WAYLAND_CLIENT)
endfunction()
