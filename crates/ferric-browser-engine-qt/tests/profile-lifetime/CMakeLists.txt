cmake_minimum_required(VERSION 3.20)
project(ferric_profile_lifetime_tests LANGUAGES CXX)
set(CMAKE_AUTOMOC ON)
set(CMAKE_CXX_STANDARD 17)
find_package(Qt6 6.11.2 EXACT REQUIRED COMPONENTS Quick WebEngineQuick Test)
add_executable(profile_lifetime_tests
    tst_lifetime.cpp ../../src/profile_lifetime.h ../../src/profile_lifetime.cpp)
target_link_libraries(profile_lifetime_tests PRIVATE Qt6::Quick Qt6::WebEngineQuick Qt6::Test)
enable_testing()
add_test(NAME profile_lifetime COMMAND profile_lifetime_tests
    pendingWindowCannotSupplyPolicyOrOwnAView
    disposalDestroysPagesBeforeTheirProfile
    profileFactoryRetainsOwnershipUntilPagesAreDestroyed
    transferredViewSurvivesOriginalQObjectDestruction
    finalHostDestructionRetiresItsOwnedView
    pendingFinalReleaseCanAcceptANewConsumer
    retiredViewCannotBeResurrectedBeforeDeferredDeletion
    destroyingTrackerCancelsConsumerCallbacks
    destroyedUiPublishesTheClearedPolicyOwner)
set_tests_properties(profile_lifetime PROPERTIES TIMEOUT 30
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QPA_PLATFORMTHEME=;QT_QUICK_BACKEND=software")
add_test(NAME everyday_T014c_popup_delegate_lifetime
    COMMAND profile_lifetime_tests popupDelegateLifetime)
set_tests_properties(everyday_T014c_popup_delegate_lifetime PROPERTIES
    TIMEOUT 180
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QPA_PLATFORMTHEME=;QT_QUICK_BACKEND=software")
