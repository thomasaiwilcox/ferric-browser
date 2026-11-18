cmake_minimum_required(VERSION 3.20)
project(ferric_native_input_lifetime LANGUAGES CXX)
set(CMAKE_AUTOMOC ON)
set(CMAKE_CXX_STANDARD 17)
find_package(Qt6 6.11.2 EXACT REQUIRED COMPONENTS Quick Test WebEngineCore)
add_executable(native_input_lifetime tst_native_input_lifetime.cpp ../../src/browser_key_router.cpp ../../src/browser_key_router.h ../../src/hint_frame_broker.cpp)
target_link_libraries(native_input_lifetime PRIVATE Qt6::Quick Qt6::Test Qt6::WebEngineCore)
target_compile_options(native_input_lifetime PRIVATE -fsanitize=address -fno-omit-frame-pointer -g)
target_link_options(native_input_lifetime PRIVATE -fsanitize=address)
enable_testing()
add_test(NAME native_input_lifetime COMMAND native_input_lifetime)
# Qt/plugin allocations are outside this focused use-after-free check. Existing
# observer/lease cycle suites retain their separate leak/retention assertions.
set_tests_properties(native_input_lifetime PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_QPA_PLATFORMTHEME=;ASAN_OPTIONS=detect_leaks=0:abort_on_error=1")
