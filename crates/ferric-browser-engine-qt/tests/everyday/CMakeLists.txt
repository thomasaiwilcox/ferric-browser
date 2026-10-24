# Included by tests/application after its production application_tests target
# has been declared.  No second browser or copied QML implementation is built.
if(NOT TARGET application_tests)
    message(FATAL_ERROR "everyday harness must use the production application_tests target")
endif()

find_package(Python3 REQUIRED COMPONENTS Interpreter)
find_package(Qt6 REQUIRED COMPONENTS Test WebChannel)
find_program(FERRIC_TEST_OPENSSL openssl REQUIRED)

add_executable(credential_boundary_tests tst_credential_boundary.cpp)
target_link_libraries(credential_boundary_tests PRIVATE "${FERRIC_ENGINE_LIBRARY}"
    Qt6::Quick Qt6::WebEngineQuick Qt6::Network Qt6::WebChannel Qt6::Test)
target_compile_definitions(credential_boundary_tests PRIVATE
    FERRIC_CREDENTIAL_FIXTURE_DIR="${CMAKE_CURRENT_SOURCE_DIR}/fixtures"
    FERRIC_TEST_OPENSSL="${FERRIC_TEST_OPENSSL}")
add_test(NAME everyday_T046_credential_isolated_world
    COMMAND $<TARGET_FILE:credential_boundary_tests> isolatedTransportAndMainFrameFacts)
set_tests_properties(everyday_T046_credential_isolated_world PROPERTIES
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

add_test(NAME everyday_T014_popup_transfer_preservation
    COMMAND $<TARGET_FILE:application_tests> productPolishPopupCommands productPolishSecondaryTabs)
set_tests_properties(everyday_T014_popup_transfer_preservation PROPERTIES
    TIMEOUT 600 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")

add_test(NAME everyday_T015_document_owner_callbacks
    COMMAND $<TARGET_FILE:application_tests> productPolishLiveTitles)
set_tests_properties(everyday_T015_document_owner_callbacks PROPERTIES
    TIMEOUT 600 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
