cmake_minimum_required(VERSION 3.20)
project(ferric_application_tests LANGUAGES CXX)
set(CMAKE_AUTOMOC ON)
set(CMAKE_CXX_STANDARD 17)
find_package(KF6WindowSystem 6.28 REQUIRED)
find_package(SQLite3 REQUIRED)
if(NOT TARGET SQLite3::SQLite3) # CMake < 4.3
    add_library(SQLite3::SQLite3 ALIAS SQLite::SQLite3)
endif()
find_package(Qt6 REQUIRED COMPONENTS Quick QuickControls2 WebEngineQuick Network DBus Test Widgets)
get_filename_component(REPO_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../../../.." ABSOLUTE)
set(FERRIC_ENGINE_LIBRARY "${REPO_ROOT}/target/release/libferric_browser_engine_qt.a" CACHE FILEPATH "Cargo-built production engine")
# Exercise the Linux distribution default locally, too. The static engine must
# precede its native shared-library dependencies so --as-needed retains them.
if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    add_link_options("LINKER:--as-needed")
endif()
add_executable(application_tests tst_application.cpp tst_permission_scope.cpp tst_notifications.cpp tst_file_choosers.cpp tst_action_feedback.cpp tst_hint_controls.cpp tst_navigation_usability.cpp tst_page_search.cpp tst_privacy_identity.cpp tst_live_titles.cpp tst_tab_recovery.cpp tst_last_tab_undo.cpp tst_open_targets.cpp tst_repeat.cpp tst_application.h)
target_link_libraries(application_tests PRIVATE "${FERRIC_ENGINE_LIBRARY}" KF6::WindowSystem Qt6::DBus Qt6::Quick Qt6::QuickControls2 Qt6::WebEngineQuick Qt6::Network Qt6::DBus Qt6::Test Qt6::Widgets SQLite3::SQLite3 ${CMAKE_DL_LIBS} pthread)
add_executable(context_reload_tests tst_context_reload.cpp)
target_link_libraries(context_reload_tests PRIVATE "${FERRIC_ENGINE_LIBRARY}" KF6::WindowSystem Qt6::DBus Qt6::Quick Qt6::QuickControls2 Qt6::WebEngineQuick Qt6::Network Qt6::DBus Qt6::Test Qt6::Widgets SQLite3::SQLite3 ${CMAKE_DL_LIBS} pthread)
# A separate, offscreen-only Qt input plugin lets the public CLI fixture
# confirm its own Quit prompt without changing the production command contract.
add_library(context_quit_test_plugin MODULE context_quit_test_plugin.cpp)
target_link_libraries(context_quit_test_plugin PRIVATE Qt6::Quick Qt6::Test)
set_target_properties(context_quit_test_plugin PROPERTIES LIBRARY_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/plugins/generic")
add_executable(switcher_tests tst_switcher.cpp)
target_link_libraries(switcher_tests PRIVATE "${FERRIC_ENGINE_LIBRARY}" KF6::WindowSystem Qt6::DBus Qt6::Quick Qt6::QuickControls2 Qt6::WebEngineQuick Qt6::Network Qt6::Test Qt6::Widgets SQLite3::SQLite3 ${CMAKE_DL_LIBS} pthread)
add_executable(downloads_tests tst_downloads.cpp)
add_library(download_save_interposer SHARED download_save_interposer.cpp)
target_link_libraries(download_save_interposer PRIVATE ${CMAKE_DL_LIBS})
add_library(download_runtime_interposer SHARED download_runtime_interposer.cpp)
add_dependencies(downloads_tests download_save_interposer download_runtime_interposer)
target_link_libraries(downloads_tests PRIVATE "${FERRIC_ENGINE_LIBRARY}" KF6::WindowSystem Qt6::DBus Qt6::Quick Qt6::QuickControls2 Qt6::WebEngineQuick Qt6::Network Qt6::Test Qt6::Widgets SQLite3::SQLite3 ${CMAKE_DL_LIBS} pthread)
add_executable(close_request_tests tst_close_requests.cpp)
target_link_libraries(close_request_tests PRIVATE "${FERRIC_ENGINE_LIBRARY}" KF6::WindowSystem Qt6::DBus Qt6::Quick Qt6::QuickControls2 Qt6::WebEngineQuick Qt6::Network Qt6::Test Qt6::Widgets SQLite3::SQLite3 ${CMAKE_DL_LIBS} pthread)
enable_testing()
add_test(NAME application_switcher_live_updates COMMAND application_tests productPolishSwitcherLiveUpdates)
set_tests_properties(application_switcher_live_updates PROPERTIES TIMEOUT 180 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_switcher_navigation COMMAND application_tests productPolishSwitcherNavigation)
set_tests_properties(application_switcher_navigation PROPERTIES TIMEOUT 180 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_live_titles COMMAND application_tests productPolishLiveTitles)
set_tests_properties(application_live_titles PROPERTIES TIMEOUT 180 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_privacy_identity COMMAND application_tests productPolishPrivacyIdentity)
set_tests_properties(application_privacy_identity PROPERTIES TIMEOUT 180 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_page_search COMMAND application_tests pageSearchFollowing pageSearchNativeText pageSearchCrossOriginFrames pageSearchCancellationAndBindings pageSearchEditorPreedit pageSearchFeedback pageSearchDirection deferredNativeRequestsKeepCapturedTargets)
set_tests_properties(application_page_search PROPERTIES TIMEOUT 180 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_navigation_usability COMMAND application_tests navigationJourneys urlCopyStatusBar bottomChromeOverlay productPolishNativePageHistory productPolishSecondaryHistoryCommands)
set_tests_properties(application_navigation_usability PROPERTIES TIMEOUT 300 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_navigation_recovery COMMAND application_tests navigationFailureRecovery productPolishNavigationFailure)
set_tests_properties(application_navigation_recovery PROPERTIES TIMEOUT 180 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_keyboard_guide COMMAND application_tests keyboardGuideOwner)
set_tests_properties(application_keyboard_guide PROPERTIES TIMEOUT 180 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_tab_recovery COMMAND application_tests productPolishTabRecovery productPolishTabRecoveryCheckpoint)
add_test(NAME application_last_tab_undo COMMAND application_tests productPolishLastTabUndo)
set_tests_properties(application_last_tab_undo PROPERTIES TIMEOUT 300 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
set_tests_properties(application_tab_recovery PROPERTIES TIMEOUT 480 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_stop_state COMMAND application_tests productPolishStopState)
set_tests_properties(application_stop_state PROPERTIES TIMEOUT 300 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_form_reload COMMAND application_tests streamingFormReload)
set_tests_properties(application_form_reload PROPERTIES TIMEOUT 300 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_streaming_history COMMAND application_tests streamingHistoryTraversal)
set_tests_properties(application_streaming_history PROPERTIES TIMEOUT 360 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_hint_controls COMMAND application_tests hintNativeControls hintNavigationLifecycle embeddedResourceParity realHintKeyboardActivation hintRefreshRetainsInput hintRefreshDoesNotAutoFollow hintConfirmationFreezesPendingRefresh scopedHintAndSearchJourneys)
set_tests_properties(application_hint_controls PROPERTIES TIMEOUT 240 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_hint_controls_dpr2 COMMAND application_tests hintNativeControls)
set_tests_properties(application_hint_controls_dpr2 PROPERTIES TIMEOUT 180 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QT_SCALE_FACTOR=2;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_close_requests COMMAND close_request_tests)
set_tests_properties(application_close_requests PROPERTIES TIMEOUT 120 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_switcher COMMAND switcher_tests)
set_tests_properties(application_switcher PROPERTIES TIMEOUT 360 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_subdirectory(../portal portal)
add_executable(file_chooser_backend_tests tst_file_chooser_backend.cpp ../../src/native_file_chooser.cpp ../../src/native_file_chooser.h ../../src/portal_file_request.cpp ../../src/portal_file_request.h ../../src/gui_application.cpp ../../src/gui_application.h ../../src/local_file_url.cpp ../../src/local_file_url.h)
target_link_libraries(file_chooser_backend_tests PRIVATE KF6::WindowSystem Qt6::DBus Qt6::Widgets Qt6::Quick Qt6::Qml Qt6::Test)
add_test(NAME application_file_chooser_backend COMMAND file_chooser_backend_tests)
set_tests_properties(application_file_chooser_backend PROPERTIES TIMEOUT 60 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=")
add_executable(file_chooser_privacy_tests tst_file_chooser_privacy.cpp ../../src/native_file_chooser.cpp ../../src/native_file_chooser.h ../../src/portal_file_request.cpp ../../src/portal_file_request.h ../../src/gui_application.cpp ../../src/gui_application.h ../../src/local_file_url.cpp ../../src/local_file_url.h)
target_link_libraries(file_chooser_privacy_tests PRIVATE KF6::WindowSystem Qt6::DBus Qt6::Widgets Qt6::Quick Qt6::Qml Qt6::Test)
add_test(NAME application_file_chooser_privacy COMMAND file_chooser_privacy_tests)
set_tests_properties(application_file_chooser_privacy PROPERTIES TIMEOUT 60 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=")
add_test(NAME application_action_feedback COMMAND application_tests spawnCommandModeValidation scrollTargetCommandChain productPolishActionFeedback productPolishConfigurationFeedback productPolishSecondarySelectionCommand actionAvailabilityFeedback productPolishRegisteredActionFeedback productPolishHintActionFeedback productPolishRapidDownloadRejection productPolishExternalActionFeedback)
set_tests_properties(application_action_feedback PROPERTIES TIMEOUT 180 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_executable(notification_transport_tests tst_notification_transport.cpp ../../src/notification_presenter.cpp ../../src/notification_presenter.h)
target_link_libraries(notification_transport_tests PRIVATE Qt6::WebEngineCore Qt6::Qml Qt6::DBus Qt6::Test)
add_executable(notification_test_service notification_test_service.cpp)
target_link_libraries(notification_test_service PRIVATE Qt6::Core Qt6::DBus)
add_dependencies(notification_transport_tests notification_test_service)
add_dependencies(application_tests notification_test_service)
add_library(settings_sync_interposer SHARED settings_sync_interposer.cpp)
target_link_libraries(settings_sync_interposer PRIVATE ${CMAKE_DL_LIBS})
add_dependencies(application_tests settings_sync_interposer)
add_library(session_read_interposer SHARED session_read_interposer.cpp)
target_link_libraries(session_read_interposer PRIVATE ${CMAKE_DL_LIBS})
add_dependencies(application_tests session_read_interposer)
add_library(startup_recovery_test_plugin MODULE startup_recovery_test_plugin.cpp)
target_link_libraries(startup_recovery_test_plugin PRIVATE Qt6::Quick Qt6::Qml Qt6::Test)
set_target_properties(startup_recovery_test_plugin PROPERTIES LIBRARY_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/plugins/generic")
add_dependencies(application_tests startup_recovery_test_plugin)
add_test(NAME application_startup_recovery COMMAND ${CMAKE_COMMAND} -E env
    "LD_PRELOAD=$<TARGET_FILE:session_read_interposer>" $<TARGET_FILE:application_tests>
    productPolishStartupRecovery)
set_tests_properties(application_startup_recovery PROPERTIES TIMEOUT 180 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_session_recovery COMMAND ${CMAKE_COMMAND} -E env
    "LD_PRELOAD=$<TARGET_FILE:session_read_interposer>" $<TARGET_FILE:application_tests>
    productPolishSessionRecoveryFailure productPolishSessionRecoveryCancellation)
set_tests_properties(application_session_recovery PROPERTIES TIMEOUT 120 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_executable(notification_consent_tests tst_notification_consent.cpp)
target_link_libraries(notification_consent_tests PRIVATE "${FERRIC_ENGINE_LIBRARY}" KF6::WindowSystem Qt6::DBus Qt6::Quick Qt6::QuickControls2 Qt6::WebEngineQuick Qt6::Network Qt6::Test Qt6::Widgets SQLite3::SQLite3 ${CMAKE_DL_LIBS} pthread)
add_dependencies(notification_consent_tests notification_test_service)
find_program(DBUS_RUN_SESSION dbus-run-session REQUIRED)
add_test(NAME application_settings_durability COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env "LD_PRELOAD=$<TARGET_FILE:settings_sync_interposer>"
    FERRIC_SETTINGS_SYNC_FIXTURE=1 $<TARGET_FILE:application_tests> productPolishSettingDurability)
set_tests_properties(application_settings_durability PROPERTIES TIMEOUT 90 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
find_package(Python3 REQUIRED COMPONENTS Interpreter)
add_test(NAME application_startup_recovery_restart COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/run_startup_recovery.py"
    "${CMAKE_CURRENT_BINARY_DIR}" "${REPO_ROOT}/target/release/ferric-browser")
set_tests_properties(application_startup_recovery_restart PROPERTIES TIMEOUT 180)
add_test(NAME application_startup_recovery_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated
    "LD_PRELOAD=$<TARGET_FILE:session_read_interposer>" "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal"
    tab_recovery productPolishStartupRecovery)
set_tests_properties(application_startup_recovery_wayland PROPERTIES TIMEOUT 180)
add_test(NAME application_session_recovery_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated
    "LD_PRELOAD=$<TARGET_FILE:session_read_interposer>" "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal"
    tab_recovery productPolishSessionRecoveryFailure productPolishSessionRecoveryCancellation)
set_tests_properties(application_session_recovery_wayland PROPERTIES TIMEOUT 150)
add_test(NAME application_switcher_live_updates_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal" switcher_live_updates)
set_tests_properties(application_switcher_live_updates_wayland PROPERTIES TIMEOUT 210)
add_test(NAME application_switcher_navigation_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal" switcher_navigation)
set_tests_properties(application_switcher_navigation_wayland PROPERTIES TIMEOUT 210)
add_test(NAME application_live_titles_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal" live_titles)
set_tests_properties(application_live_titles_wayland PROPERTIES TIMEOUT 210)
add_dependencies(switcher_tests portal_compositor)
add_dependencies(application_tests portal_compositor)
add_test(NAME application_bottom_chrome_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal" chrome_overlay)
set_tests_properties(application_bottom_chrome_wayland PROPERTIES TIMEOUT 330)
add_test(NAME application_navigation_recovery_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal" navigation_recovery)
set_tests_properties(application_navigation_recovery_wayland PROPERTIES TIMEOUT 180)
add_test(NAME application_keyboard_guide_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal" keyboard_guide)
set_tests_properties(application_keyboard_guide_wayland PROPERTIES TIMEOUT 180)
add_test(NAME application_page_search_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal" page_search)
set_tests_properties(application_page_search_wayland PROPERTIES TIMEOUT 210)
add_test(NAME application_privacy_identity_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal" privacy_identity)
set_tests_properties(application_privacy_identity_wayland PROPERTIES TIMEOUT 210)
add_test(NAME application_tab_recovery_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal" tab_recovery)
set_tests_properties(application_tab_recovery_wayland PROPERTIES TIMEOUT 480)
add_test(NAME application_last_tab_undo_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal"
    tab_recovery productPolishLastTabUndo)
set_tests_properties(application_last_tab_undo_wayland PROPERTIES TIMEOUT 300)
add_test(NAME application_stop_state_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal" stop_state)
set_tests_properties(application_stop_state_wayland PROPERTIES TIMEOUT 300)
add_test(NAME application_form_reload_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal" form_reload)
set_tests_properties(application_form_reload_wayland PROPERTIES TIMEOUT 300)
add_test(NAME application_streaming_history_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal" streaming_history)
set_tests_properties(application_streaming_history_wayland PROPERTIES TIMEOUT 360)
add_test(NAME application_command_editing_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal" command_editing)
set_tests_properties(application_command_editing_wayland PROPERTIES TIMEOUT 180)
add_test(NAME application_keyboard_chrome_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal" keyboard_chrome)
set_tests_properties(application_keyboard_chrome_wayland PROPERTIES TIMEOUT 210)
add_test(NAME application_tab_focus_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal" keyboard_chrome
    tabKeyboardContinuity)
set_tests_properties(application_tab_focus_wayland PROPERTIES TIMEOUT 210)
add_test(NAME application_download_finalization COMMAND "${DBUS_RUN_SESSION}" --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf -- "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/run_download_finalization_probe.py" --executable $<TARGET_FILE:downloads_tests> --interposer $<TARGET_FILE:download_save_interposer>)
set_tests_properties(application_download_finalization PROPERTIES TIMEOUT 100)
set(FERRIC_BROWSER_EXECUTABLE "${REPO_ROOT}/target/release/ferric-browser" CACHE FILEPATH "Production browser for runtime-version admission regression")
add_test(NAME application_journey_history COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/run_journey_history_probe.py"
    --executable "${FERRIC_BROWSER_EXECUTABLE}"
    --artifacts "${CMAKE_CURRENT_BINARY_DIR}/journey-history-artifacts")
set_tests_properties(application_journey_history PROPERTIES TIMEOUT 180)
add_test(NAME application_download_runtime_guard COMMAND "${DBUS_RUN_SESSION}" --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf -- "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/run_download_runtime_probe.py" --executable "${FERRIC_BROWSER_EXECUTABLE}" --interposer $<TARGET_FILE:download_runtime_interposer>)
set_tests_properties(application_download_runtime_guard PROPERTIES TIMEOUT 90)
add_test(NAME application_downloads COMMAND "${DBUS_RUN_SESSION}" --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf -- $<TARGET_FILE:downloads_tests>)
set_tests_properties(application_downloads PROPERTIES TIMEOUT 180 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_notification_consent COMMAND "${DBUS_RUN_SESSION}" --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf -- ${CMAKE_COMMAND} -E env FERRIC_NOTIFICATION_TEST_BUS=isolated $<TARGET_FILE:notification_consent_tests>)
set_tests_properties(application_notification_consent PROPERTIES TIMEOUT 90 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_notification_transport COMMAND "${DBUS_RUN_SESSION}" --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf -- ${CMAKE_COMMAND} -E env FERRIC_NOTIFICATION_TEST_BUS=isolated $<TARGET_FILE:notification_transport_tests>)
set_tests_properties(application_notification_transport PROPERTIES TIMEOUT 60)
add_test(NAME application_native_notifications COMMAND "${DBUS_RUN_SESSION}" --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf -- ${CMAKE_COMMAND} -E env FERRIC_NOTIFICATION_TEST_BUS=isolated $<TARGET_FILE:application_tests> productPolishNativeNotifications)
set_tests_properties(application_native_notifications PROPERTIES TIMEOUT 180 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_context_reload COMMAND context_reload_tests)
set_tests_properties(application_context_reload PROPERTIES TIMEOUT 90 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")
add_test(NAME application_offscreen COMMAND application_tests)
# This aggregate includes hundreds of cases, each creating a browser fixture.
# Release builds exceed eight minutes on a two-core host even without failures;
# keep per-assertion deadlines and give the complete suite its own outer budget.
set_tests_properties(application_offscreen PROPERTIES TIMEOUT 1200 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")

add_test(NAME application_offscreen_dpr2 COMMAND application_tests scopedGridNativeCoordinates)
set_tests_properties(application_offscreen_dpr2 PROPERTIES TIMEOUT 90 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QT_SCALE_FACTOR=2;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")

add_test(NAME application_lifetime COMMAND application_tests productionHintLifetimeCycles)
set_tests_properties(application_lifetime PROPERTIES TIMEOUT 300 ENVIRONMENT "FERRIC_APP_STRESS=1;QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu --js-flags=--expose-gc")

add_test(NAME application_frame_dpr2 COMMAND application_tests crossOriginConsentHints)
set_tests_properties(application_frame_dpr2 PROPERTIES TIMEOUT 120 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QT_SCALE_FACTOR=2;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")

add_test(NAME application_frame_lifetime COMMAND application_tests crossOriginFrameLifetimeCycles)
set_tests_properties(application_frame_lifetime PROPERTIES TIMEOUT 700 ENVIRONMENT "QTEST_FUNCTION_TIMEOUT=600000;FERRIC_APP_STRESS=1;QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu --js-flags=--expose-gc")

add_test(NAME application_clipboard COMMAND application_tests clipboardNativeEditing clipboardChrome clipboardMenus clipboardSelectionOwnership clipboardShortcutOverride clipboardSelectionControls clipboardSelectionChangedRejects clipboardSelectionLifetimeCycles clipboardHeldBrowserGesture clipboardOtherWindows)
set_tests_properties(application_clipboard PROPERTIES TIMEOUT 360 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")

add_test(NAME application_product_polish COMMAND application_tests embeddedResourceParity targetSearchNeedsConfirmation promptAcceptCancelAndNestedRestoration productPolishPagePrompts productPolishWindowPrompts productPolishTlsPrompt productPolishCertificateGuards productPolishIdentityWindows productPolishPromptTeardown productPolishHttpAuthentication productPolishFileChooser productPolishDownloadSelectionFailure productPolishDownloadIdentity productPolishWebAuthStates productPolishNavigationFailure productPolishManagementSurfaces productPolishWindowSettings productPolishNativePageHistory productPolishSecondaryHistoryCommands productPolishTransferredSourceClose productPolishPopupSourceClose productPolishPopupCommands productPolishSecondaryHints productPolishCopyFeedback productPolishNewWindows productPolishStartupCleanLink productPolishCleanLinkOwner productPolishDevToolsOwner productPolishInteractiveNewWindow productPolishSessionPermissionSharing productPolishCombinedSessionPermissionReset productPolishPermissionReadiness productPolishPermissionRevocationScope productPolishPermissionResetOwner productPolishPermissionResetQueue productPolishPendingPermissionPolicy productPolishPermissionQueueBound productPolishPermissionCapacity productPolishPopupDurableWork productPolishNotificationResetScope productPolishContextCatalogBootstrap productPolishContextCatalogRetainedAfterFailedProfile productPolishNewWindowRouting productPolishProvisionalTransferFallback productPolishSecondaryTabs productPolishSecondaryManagers productPolishIndependentWindowInput productPolishPermissionWindowOwnership productPolishOverlappingModalInput productPolishUserscriptFeedback productPolishWindowClose productPolishPopupScripts productPolishSiteLedger productPolishSessionPreview productPolishSessionSaveBackpressure productPolishManagerFeedback productPolishLibrary productPolishPrivateLibrary productPolishStartupProblems productPolishProfileRecovery productPolishFailedProfileDiagnostics productPolishDiagnosticOwners productPolishDiagnosticReport productPolishDiagnosticLifecycle productPolishLocalFilePaths productPolishUnavailableStorage productPolishBookmarkTransfer)
set_tests_properties(application_product_polish PROPERTIES TIMEOUT 600 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")

add_test(NAME application_deferred_requests COMMAND application_tests deferredNativeRequestsKeepCapturedTargets)
set_tests_properties(application_deferred_requests PROPERTIES TIMEOUT 90 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")

add_test(NAME application_repeat COMMAND application_tests repeatAdmission repeatCommonCommands repeatKeyboardCount repeatWindowRouting)
set_tests_properties(application_repeat PROPERTIES TIMEOUT 240 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")

add_test(NAME application_open_targets COMMAND application_tests productPolishOpenTargets)
set_tests_properties(application_open_targets PROPERTIES TIMEOUT 240 ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu")

add_test(NAME application_open_targets_wayland COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    ${CMAKE_COMMAND} -E env FERRIC_PORTAL_TEST_BUS=isolated "${Python3_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../portal/run.py" "${CMAKE_CURRENT_BINARY_DIR}/portal"
    tab_recovery productPolishOpenTargets)
set_tests_properties(application_open_targets_wayland PROPERTIES TIMEOUT 240)
add_test(NAME application_open_targets_cli COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/run_open_targets.py"
    --browser "${FERRIC_BROWSER_EXECUTABLE}" --output "${CMAKE_CURRENT_BINARY_DIR}/open-targets-cli")
set_tests_properties(application_open_targets_cli PROPERTIES TIMEOUT 180)

add_test(NAME application_cli_output COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    "${Python3_EXECUTABLE}" "${REPO_ROOT}/crates/ferric-browser/tests/run_cli_output.py"
    --browser "${FERRIC_BROWSER_EXECUTABLE}" --output "${CMAKE_CURRENT_BINARY_DIR}/cli-output")
set_tests_properties(application_cli_output PROPERTIES TIMEOUT 180)

add_test(NAME application_context_selection_cli COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    "${Python3_EXECUTABLE}" "${REPO_ROOT}/crates/ferric-browser/tests/run_context_selection.py"
    --browser "${FERRIC_BROWSER_EXECUTABLE}" --output "${CMAKE_CURRENT_BINARY_DIR}/context-selection-cli")
set_tests_properties(application_context_selection_cli PROPERTIES TIMEOUT 240)

add_test(NAME application_repeat_cli COMMAND "${DBUS_RUN_SESSION}"
    --config-file=${CMAKE_CURRENT_SOURCE_DIR}/notification-test-bus.conf --
    "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/run_repeat.py"
    --browser "${FERRIC_BROWSER_EXECUTABLE}" --output "${CMAKE_CURRENT_BINARY_DIR}/repeat-cli")
set_tests_properties(application_repeat_cli PROPERTIES TIMEOUT 180)
