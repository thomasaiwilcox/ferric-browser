cmake_minimum_required(VERSION 3.21)
project(ferric_public_page_host LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

find_package(Qt6 6.11.2 EXACT REQUIRED COMPONENTS
    Core Gui Widgets Quick QuickControls2 QuickWidgets Network WebEngineCore WebEngineQuick WebEngineWidgets Test)
find_package(SQLite3 REQUIRED)

set(FERRIC_ENGINE_LIBRARY "" CACHE FILEPATH "Exact Cargo-built Ferric Qt engine archive")

include(CTest)
if(BUILD_TESTING)
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
    add_executable(public_page_owner_tests tst_public_page_owner.cpp)
    target_include_directories(public_page_owner_tests PRIVATE
        "${CMAKE_CURRENT_LIST_DIR}/../../src")
    target_link_libraries(public_page_owner_tests PRIVATE
        "${FERRIC_ENGINE_LIBRARY}"
        SQLite::SQLite3
        Qt6::Quick Qt6::Widgets Qt6::QuickControls2 Qt6::DBus
        Qt6::Core Qt6::Gui Qt6::Network Qt6::WebEngineCore Qt6::WebEngineQuick
        Qt6::Test)
    add_test(NAME T014e1_public_page_owner COMMAND public_page_owner_tests)
    set_tests_properties(T014e1_public_page_owner PROPERTIES
        TIMEOUT 180 LABELS "component;webengine")

    set_tests_properties(public_page_host_native_input PROPERTIES
        LABELS "native;wayland"
        TIMEOUT 45)
endif()
