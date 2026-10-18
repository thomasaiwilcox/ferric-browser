cmake_minimum_required(VERSION 3.20)
project(ferric_profile_lifetime_tests LANGUAGES CXX)
set(CMAKE_AUTOMOC ON)
set(CMAKE_CXX_STANDARD 17)
find_package(Qt6 REQUIRED COMPONENTS Quick Test)
add_executable(profile_lifetime_tests
    tst_lifetime.cpp ../../src/profile_lifetime.h ../../src/profile_lifetime.cpp)
target_link_libraries(profile_lifetime_tests PRIVATE Qt6::Quick Qt6::Test)
enable_testing()
add_test(NAME profile_lifetime COMMAND profile_lifetime_tests)
set_tests_properties(profile_lifetime PROPERTIES TIMEOUT 30
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QPA_PLATFORMTHEME=;QT_QUICK_BACKEND=software")
