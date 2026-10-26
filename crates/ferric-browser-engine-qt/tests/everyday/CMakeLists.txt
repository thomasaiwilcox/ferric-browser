# Included by tests/application after its production application_tests target
# has been declared.  No second browser or copied QML implementation is built.
if(NOT TARGET application_tests)
    message(FATAL_ERROR "everyday harness must use the production application_tests target")
endif()

find_package(Python3 REQUIRED COMPONENTS Interpreter)
find_package(Qt6 REQUIRED COMPONENTS Core DBus Test WebChannel)
find_program(FERRIC_TEST_OPENSSL openssl REQUIRED)
get_target_property(_ferric_application_test_links application_tests LINK_LIBRARIES)

add_executable(everyday_T061a_helper_supervisor_component tst_credential_process.cpp)
target_link_libraries(everyday_T061a_helper_supervisor_component PRIVATE
    "${FERRIC_ENGINE_LIBRARY}" Qt6::Core Qt6::Test)
add_test(NAME everyday_T061a_helper_supervisor_component
    COMMAND $<TARGET_FILE:everyday_T061a_helper_supervisor_component> helperSupervisor)
set_tests_properties(everyday_T061a_helper_supervisor_component PROPERTIES TIMEOUT 45)

add_executable(credential_boundary_tests tst_credential_boundary.cpp)
target_link_libraries(credential_boundary_tests PRIVATE "${FERRIC_ENGINE_LIBRARY}"
    ${_ferric_application_test_links} Qt6::DBus Qt6::Quick Qt6::WebEngineQuick
    Qt6::Network Qt6::WebChannel Qt6::Test)
target_compile_definitions(credential_boundary_tests PRIVATE
    FERRIC_CREDENTIAL_FIXTURE_DIR="${CMAKE_CURRENT_SOURCE_DIR}/fixtures"
    FERRIC_TEST_OPENSSL="${FERRIC_TEST_OPENSSL}")
add_test(NAME everyday_T046_credential_isolated_world
    COMMAND $<TARGET_FILE:credential_boundary_tests> isolatedTransportAndMainFrameFacts)
set_tests_properties(everyday_T046_credential_isolated_world PROPERTIES
    TIMEOUT 90 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME everyday_T047_stale_credential_completion
    COMMAND $<TARGET_FILE:credential_boundary_tests> staleCredentialCompletionAdmission)
set_tests_properties(everyday_T047_stale_credential_completion PROPERTIES
    TIMEOUT 90 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")

add_executable(everyday_fixture_tests tst_everyday.cpp tst_everyday.h)
target_link_libraries(everyday_fixture_tests PRIVATE Qt6::Test)
add_test(NAME everyday_fixture_http_https_isolation COMMAND everyday_fixture_tests)
set_tests_properties(everyday_fixture_http_https_isolation PROPERTIES
    TIMEOUT 20 ENVIRONMENT "FERRIC_EVERYDAY_FIXTURE_SERVER=${CMAKE_CURRENT_SOURCE_DIR}/fixture_server.py;Python3_EXECUTABLE=${Python3_EXECUTABLE}")

add_test(NAME everyday_T004_production_navigation
    COMMAND $<TARGET_FILE:application_tests> navigationJourneys)
set_tests_properties(everyday_T004_production_navigation PROPERTIES
    TIMEOUT 300 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")

add_test(NAME everyday_T004_production_search_and_hints
    COMMAND $<TARGET_FILE:application_tests>
        pageSearchFollowing pageSearchNativeText hintNativeControls hintNavigationLifecycle)
set_tests_properties(everyday_T004_production_search_and_hints PROPERTIES
    TIMEOUT 300 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")

# This test enumerates the generated CTest inventory and fails if the expected
# acceptance executables disappear or if the inventory is empty.
add_test(NAME everyday_T005_discovery
    COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/fixture_server.py"
        --check-discovery "${CMAKE_BINARY_DIR}" --filter "^everyday_T004_production_navigation$")
set_tests_properties(everyday_T005_discovery PROPERTIES TIMEOUT 30)

# The production privacy-identity case creates isolated profile roots and does
# not use the user's Ferric profile.
add_test(NAME everyday_T006_disposable_profile_isolation
    COMMAND $<TARGET_FILE:application_tests>
        productPolishPrivacyIdentity)
set_tests_properties(everyday_T006_disposable_profile_isolation PROPERTIES
    TIMEOUT 240 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")

add_test(NAME everyday_T006_isolated_portal_fixture
    COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/fixture_server.py"
        --run-filter "${CMAKE_BINARY_DIR}" --filter "^portal_activation$")
set_tests_properties(everyday_T006_isolated_portal_fixture PROPERTIES TIMEOUT 100)

# F02 acceptance coverage exercises the production application harness and the
# shared page factory through cold creation, native popup handoff/transfer, and
# captured document-title callbacks.
add_test(NAME everyday_T013_factory_equivalence
    COMMAND $<TARGET_FILE:application_tests> productPolishSecondaryTabs)
set_tests_properties(everyday_T013_factory_equivalence PROPERTIES
    TIMEOUT 300 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")

add_test(NAME everyday_T014a_popup_commands_opener_identity
    COMMAND $<TARGET_FILE:application_tests> productPolishPopupCommandsOpenerIdentity)
set_tests_properties(everyday_T014a_popup_commands_opener_identity PROPERTIES
    TIMEOUT 600 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")

add_test(NAME everyday_T015_document_owner_callbacks
    COMMAND $<TARGET_FILE:application_tests> productPolishLiveTitles)
set_tests_properties(everyday_T015_document_owner_callbacks PROPERTIES
    TIMEOUT 600 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")

set(FERRIC_ENGINE_CXX_INCLUDE_DIR "" CACHE PATH
    "Exact CXX-Qt generated include directory for the selected engine build")
if(NOT EXISTS "${FERRIC_ENGINE_CXX_INCLUDE_DIR}/ferric-browser-engine-qt/src/bridge.cxxqt.h")
  message(FATAL_ERROR "FERRIC_ENGINE_CXX_INCLUDE_DIR must name the selected engine cxxqtbuild/include directory")
endif()
add_executable(credential_process_bridge_tests tst_credential_process_bridge.cpp)
target_include_directories(credential_process_bridge_tests PRIVATE
  "${CMAKE_CURRENT_LIST_DIR}/../../src"
  "${FERRIC_ENGINE_CXX_INCLUDE_DIR}")
target_link_libraries(credential_process_bridge_tests PRIVATE
  "${FERRIC_ENGINE_LIBRARY}" ${_ferric_application_test_links})
add_test(NAME everyday_T061c_helper_native_rust_bridge
  COMMAND credential_process_bridge_tests nativeRustBridge)
set_tests_properties(everyday_T061c_helper_native_rust_bridge PROPERTIES
  ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software")

target_include_directories(everyday_T061a_helper_supervisor_component PRIVATE "${FERRIC_ENGINE_CXX_INCLUDE_DIR}")

target_link_libraries(everyday_T061a_helper_supervisor_component PRIVATE ${_ferric_application_test_links})
