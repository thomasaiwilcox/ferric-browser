cmake_minimum_required(VERSION 3.22)
project(FerricEngineCookieProbe LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
find_package(Qt6 6.11.2 EXACT REQUIRED COMPONENTS Core Gui Widgets Network Qml Quick WebEngineCore WebEngineQuick)
add_executable(ferric-engine-cookie-probe cookie_probe.cpp)
add_executable(ferric-engine-cookie-request-probe cookie_request_probe.cpp)
add_executable(ferric-engine-rtc-registry-probe rtc_registry_probe.cpp)
add_executable(ferric-engine-rtc-quick-bridge-probe rtc_quick_bridge_probe.cpp)
add_executable(ferric-engine-legacy-cookie-probe legacy_cookie_probe.cpp)
# Qt's forwarding headers include <QtWebEngineCore/...>. Put this patched
# module's include root before the system QtCore/Gui roots, which also contain
# the unpatched WebEngine headers on this host.
foreach(probe IN ITEMS ferric-engine-cookie-probe ferric-engine-cookie-request-probe ferric-engine-rtc-registry-probe ferric-engine-rtc-quick-bridge-probe ferric-engine-legacy-cookie-probe)
    target_link_libraries(${probe} PRIVATE
        Qt6::WebEngineCore Qt6::Widgets Qt6::Gui Qt6::Network Qt6::Core)
    target_compile_options(${probe} PRIVATE -Wall -Wextra -Werror)
endforeach()
target_link_libraries(ferric-engine-rtc-quick-bridge-probe PRIVATE
    Qt6::WebEngineQuick Qt6::Quick Qt6::Qml Qt6::Gui Qt6::Network Qt6::Core)
target_compile_definitions(ferric-engine-rtc-quick-bridge-probe PRIVATE
    FERRIC_RTC_QUICK_QML_PATH="${CMAKE_CURRENT_SOURCE_DIR}/rtc_quick_bridge_probe.qml")
# Deliberately separate from Ferric/xtask: this executable requires the patched
# experimental Qt build and must never be linked into the shipping adapter.
