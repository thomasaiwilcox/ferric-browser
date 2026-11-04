cmake_minimum_required(VERSION 3.21)
project(ferric_public_page_host LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 17)
find_package(Qt6GuiPrivate 6.11.2 EXACT REQUIRED)
find_package(Qt6 6.11.2 EXACT REQUIRED COMPONENTS WaylandClient)
include(../../protocols/Protocols.cmake)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

find_package(Qt6 6.11.2 EXACT REQUIRED COMPONENTS WaylandClient
    Core Gui Widgets Quick QuickControls2 QuickWidgets Network DBus WebChannel WebEngineCore WebEngineQuick WebEngineWidgets Test)
find_package(SQLite3 REQUIRED)
find_program(FERRIC_TEST_OPENSSL openssl REQUIRED)
find_package(Python3 REQUIRED COMPONENTS Interpreter)

set(FERRIC_ENGINE_LIBRARY "" CACHE FILEPATH "Exact Cargo-built Ferric Qt engine archive")

include(CTest)
if(BUILD_TESTING)
    add_executable(credential_network_policy_tests tst_credential_network_policy.cpp
        ../../src/credential_network_policy.cpp ../../src/credential_network_policy.h)
    target_include_directories(credential_network_policy_tests PRIVATE ../../src)
    target_link_libraries(credential_network_policy_tests PRIVATE Qt6::Core Qt6::Network Qt6::Test)
    add_test(NAME credential_network_startup_policy COMMAND credential_network_policy_tests)
    set_tests_properties(credential_network_startup_policy PROPERTIES TIMEOUT 30 LABELS "credential;network")

    add_executable(quick_credential_surface_tests tst_quick_credential_surface.cpp
        ../../src/quick_credential_owner.h ../../src/quick_credential_owner.cpp
        ../../src/credential_boundary.h ../../src/credential_boundary.cpp)
    target_include_directories(quick_credential_surface_tests PRIVATE ../../src)
    target_link_libraries(quick_credential_surface_tests PRIVATE
        Qt6::Core Qt6::Gui Qt6::Widgets Qt6::Quick Qt6::Network
        Qt6::WebEngineCore Qt6::WebEngineQuick Qt6::WaylandClient Qt6::WebChannel Qt6::Test)
    target_compile_definitions(quick_credential_surface_tests PRIVATE
        FERRIC_TEST_OPENSSL="${FERRIC_TEST_OPENSSL}"
        FERRIC_CREDENTIAL_SCRIPTS="${CMAKE_CURRENT_LIST_DIR}/../../qml/scripts/credential")
    add_test(NAME quick_credential_public_surface COMMAND quick_credential_surface_tests)
    add_test(NAME quick_credential_helper_pipeline COMMAND "${Python3_EXECUTABLE}"
        "${CMAKE_CURRENT_LIST_DIR}/run_phone_actor_fixture.py" "$<TARGET_FILE:quick_credential_surface_tests>"
        --scratch-root "${CMAKE_CURRENT_BINARY_DIR}")
    add_test(NAME quick_credential_helper_mapped_pipeline COMMAND "${Python3_EXECUTABLE}"
        "${CMAKE_CURRENT_LIST_DIR}/run_phone_actor_fixture.py" "$<TARGET_FILE:quick_credential_surface_tests>"
        --scratch-root "${CMAKE_CURRENT_BINARY_DIR}")
    set_tests_properties(quick_credential_helper_mapped_pipeline PROPERTIES TIMEOUT 210 LABELS "credential;wayland;qml;synthetic-helper"
        ENVIRONMENT "QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu --disable-gpu-compositing --host-resolver-rules=\"MAP localhost 127.0.0.1\"")
    add_test(NAME credential_fixture_ownership COMMAND "${Python3_EXECUTABLE}"
        "${CMAKE_CURRENT_LIST_DIR}/test_phone_actor_runner.py")
    set_tests_properties(credential_fixture_ownership PROPERTIES TIMEOUT 60 LABELS "credential;synthetic-helper")
    set_tests_properties(quick_credential_helper_pipeline PROPERTIES TIMEOUT 210 LABELS "credential;wayland;qml;synthetic-helper")
    set_tests_properties(quick_credential_public_surface PROPERTIES TIMEOUT 90 LABELS "credential;wayland;qml")

    add_executable(window_scene_tests tst_window_scene.cpp
        ../../src/window_scene.h ../../src/window_scene.cpp)
    target_include_directories(window_scene_tests PRIVATE ../../src)
    target_link_libraries(window_scene_tests PRIVATE
        Qt6::Core Qt6::Gui Qt6::Widgets Qt6::Quick Qt6::QuickControls2
        Qt6::QuickWidgets Qt6::Test)
    add_test(NAME public_window_shared_scene COMMAND window_scene_tests)
    set_tests_properties(public_window_shared_scene PROPERTIES
        TIMEOUT 60 LABELS "native;wayland;qml")

    add_executable(public_page_host_tests tst_public_page_host.cpp)
    target_link_libraries(public_page_host_tests PRIVATE
        Qt6::Core Qt6::Gui Qt6::Widgets Qt6::Quick Qt6::QuickWidgets
        Qt6::Network Qt6::WebEngineCore Qt6::WebEngineWidgets Qt6::Test)
    target_compile_definitions(public_page_host_tests PRIVATE
        FIXTURE_DIR="${CMAKE_CURRENT_SOURCE_DIR}/fixtures"
        CHROME_QML="${CMAKE_CURRENT_SOURCE_DIR}/chrome.qml")

    add_test(NAME public_page_host_lifecycle COMMAND public_page_host_tests lifecycle)
    add_test(NAME public_page_host_profiles COMMAND public_page_host_tests profiles)
    add_test(NAME public_page_host_dialogs COMMAND public_page_host_tests dialogs)
    add_test(NAME public_page_host_failure_cleanup COMMAND public_page_host_tests failureCleanup)
    add_test(NAME public_page_host_native_input COMMAND public_page_host_tests nativeInput)
    set_tests_properties(public_page_host_lifecycle public_page_host_profiles
        public_page_host_dialogs public_page_host_failure_cleanup PROPERTIES
        TIMEOUT 90)

    if(NOT FERRIC_ENGINE_LIBRARY OR NOT EXISTS "${FERRIC_ENGINE_LIBRARY}")
        message(FATAL_ERROR "FERRIC_ENGINE_LIBRARY must name the exact Cargo-built engine archive")
    endif()
    add_executable(public_page_owner_tests tst_public_page_owner.cpp
        ../../src/native_page_surface.h ../../src/native_page_surface.cpp)
    target_include_directories(public_page_owner_tests PRIVATE
        "${CMAKE_CURRENT_LIST_DIR}/../../src")
    target_link_libraries(public_page_owner_tests PRIVATE
        "${FERRIC_ENGINE_LIBRARY}"
        SQLite::SQLite3
        Qt6::Quick Qt6::Widgets Qt6::QuickControls2 Qt6::QuickWidgets Qt6::DBus
        Qt6::WebEngineWidgets Qt6::WaylandClient Qt6::WebChannel PkgConfig::FERRIC_WAYLAND_CLIENT
        Qt6::Core Qt6::Gui Qt6::Network Qt6::WebEngineCore Qt6::WebEngineQuick
        Qt6::Test)
    add_test(NAME T014e1_public_page_owner COMMAND public_page_owner_tests)
    set_tests_properties(T014e1_public_page_owner PROPERTIES
        TIMEOUT 180 LABELS "component;webengine")

    add_executable(static_qml_registration_tests tst_static_qml_registration.cpp)
    target_link_libraries(static_qml_registration_tests PRIVATE
        "${FERRIC_ENGINE_LIBRARY}"
        SQLite::SQLite3
        PkgConfig::FERRIC_WAYLAND_CLIENT
        Qt6::Core Qt6::Gui Qt6::Widgets Qt6::Quick Qt6::QuickControls2
        Qt6::QuickWidgets Qt6::Network Qt6::DBus
        Qt6::WebEngineCore Qt6::WebEngineQuick Qt6::WebEngineWidgets Qt6::WaylandClient Qt6::WebChannel Qt6::Test)
    add_test(NAME T014e2q COMMAND static_qml_registration_tests)
    set_tests_properties(T014e2q PROPERTIES
        TIMEOUT 90 LABELS "component;qml")

    add_executable(public_page_host_integration_tests
        tst_public_page_host_integration.cpp)
    get_target_property(_public_page_owner_links public_page_owner_tests LINK_LIBRARIES)
    target_link_libraries(quick_credential_surface_tests PRIVATE ${_public_page_owner_links})
    target_link_libraries(public_page_host_integration_tests PRIVATE
        ${_public_page_owner_links}
        PkgConfig::FERRIC_WAYLAND_CLIENT
        Qt6::QuickWidgets
        Qt6::WebEngineWidgets)
    target_include_directories(public_page_host_integration_tests PRIVATE
        "${CMAKE_CURRENT_LIST_DIR}/../../src")
    add_executable(phone_rp_browser phone_rp_browser.cpp)
    target_include_directories(phone_rp_browser PRIVATE "${CMAKE_CURRENT_LIST_DIR}/../../src")
    target_link_libraries(phone_rp_browser PRIVATE ${_public_page_owner_links})
    target_compile_definitions(public_page_host_integration_tests PRIVATE FERRIC_TEST_OPENSSL="${FERRIC_TEST_OPENSSL}" FERRIC_TEST_CREDENTIAL_SHIM="${CMAKE_CURRENT_LIST_DIR}/../../qml/scripts/credential/shim.js")
    add_test(NAME credential_shim_semantics COMMAND public_page_host_integration_tests credentialShimSemantics)
    set_tests_properties(credential_shim_semantics PROPERTIES TIMEOUT 120 LABELS "application;credential;webengine")
    add_test(NAME credential_production_boundary COMMAND public_page_host_integration_tests productionCredentialBoundaryAndChoice)
    add_test(NAME credential_production_lifetime COMMAND public_page_host_integration_tests productionCredentialBoundaryInvalidation)
    set_tests_properties(credential_production_boundary credential_production_lifetime PROPERTIES TIMEOUT 120 LABELS "application;credential;webengine")
    add_test(NAME public_page_captured_commands COMMAND public_page_host_integration_tests capturedCommandsUseProductionDispatcher)
    set_tests_properties(public_page_captured_commands PROPERTIES TIMEOUT 120 LABELS "application;webengine;qml")
    add_test(NAME public_page_captured_navigation COMMAND public_page_host_integration_tests
        capturedNavigationUsesProductionDispatcher)
    set_tests_properties(public_page_captured_navigation PROPERTIES TIMEOUT 90 LABELS "application;webengine;qml")
    add_test(NAME T014e2 COMMAND public_page_host_integration_tests
        rustBrowserUiRequestReparentsSamePageAcrossHosts)
    add_test(NAME T014e2r COMMAND public_page_host_integration_tests
        rustBrowserUiRequestReparentsSamePageAcrossHostsWithInitializer)
    add_test(NAME T014e2s COMMAND public_page_host_integration_tests
        rustBrowserUiRequestReparentsSamePageAcrossHostsWithProfileBootstrap)
    set_tests_properties(T014e2 T014e2r T014e2s PROPERTIES
        TIMEOUT 180 LABELS "application;webengine;qml")

    set_tests_properties(public_page_host_native_input PROPERTIES
        LABELS "native;wayland"
        TIMEOUT 45)
endif()

include("${CMAKE_CURRENT_LIST_DIR}/../EngineArchive.cmake")
ferric_require_current_engine_archive("${FERRIC_ENGINE_LIBRARY}")
