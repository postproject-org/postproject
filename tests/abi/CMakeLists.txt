cmake_minimum_required(VERSION 3.21)

project(postproject_native_consumers LANGUAGES C CXX)

find_package(PostProject 0.5 REQUIRED CONFIG)

# The producer generates this source from the public header. This consumer
# compiles it against the install prefix and never invokes Cargo.
set(POSTPROJECT_ABI_LAYOUT_SOURCE "" CACHE FILEPATH "Generated C layout probe")
if(POSTPROJECT_ABI_LAYOUT_SOURCE)
    add_executable(postproject-abi-layout "${POSTPROJECT_ABI_LAYOUT_SOURCE}")
    target_compile_features(postproject-abi-layout PRIVATE c_std_11)
    target_link_libraries(postproject-abi-layout PRIVATE PostProject::postproject)
endif()

add_executable(postproject-cpp-smoke cpp_smoke.cpp)
target_compile_features(postproject-cpp-smoke PRIVATE cxx_std_17)
target_link_libraries(postproject-cpp-smoke PRIVATE PostProject::postproject)

# The wrapper must also serve hosts that build without exceptions.
add_executable(postproject-cpp-no-exceptions cpp_no_exceptions.cpp)
target_compile_features(postproject-cpp-no-exceptions PRIVATE cxx_std_17)
target_link_libraries(postproject-cpp-no-exceptions PRIVATE PostProject::postproject)

add_executable(postproject-c-smoke c_smoke.c)
target_compile_features(postproject-c-smoke PRIVATE c_std_11)
target_link_libraries(postproject-c-smoke PRIVATE PostProject::postproject)

add_executable(postproject-c-transaction-lifetime c_transaction_lifetime.c)
target_compile_features(postproject-c-transaction-lifetime PRIVATE c_std_11)
target_link_libraries(postproject-c-transaction-lifetime PRIVATE PostProject::postproject)

add_executable(postproject-cpp-transaction-lifetime cpp_transaction_lifetime.cpp)
target_compile_features(postproject-cpp-transaction-lifetime PRIVATE cxx_std_17)
target_link_libraries(postproject-cpp-transaction-lifetime PRIVATE PostProject::postproject)

if(WIN32)
    foreach(consumer IN ITEMS postproject-c-smoke postproject-cpp-smoke
                             postproject-cpp-no-exceptions
                             postproject-c-transaction-lifetime
                             postproject-cpp-transaction-lifetime)
        add_custom_command(
            TARGET "${consumer}"
            POST_BUILD
            COMMAND "${CMAKE_COMMAND}" -E copy_if_different
                "$<TARGET_FILE:PostProject::postproject>"
                "$<TARGET_FILE_DIR:${consumer}>"
        )
    endforeach()
endif()

if(MSVC)
    target_compile_options(postproject-cpp-smoke PRIVATE /W4 /WX)
    target_compile_options(postproject-cpp-no-exceptions PRIVATE /W4 /WX /EHs-c-)
    target_compile_definitions(postproject-cpp-no-exceptions PRIVATE _HAS_EXCEPTIONS=0 _CRT_SECURE_NO_WARNINGS)
    target_compile_options(postproject-c-smoke PRIVATE /W4 /WX)
    target_compile_definitions(postproject-c-smoke PRIVATE _CRT_SECURE_NO_WARNINGS)
else()
    target_compile_options(postproject-cpp-smoke PRIVATE -Wall -Wextra -Wpedantic -Werror)
    target_compile_options(postproject-cpp-no-exceptions PRIVATE -Wall -Wextra -Wpedantic -Werror -fno-exceptions)
    target_compile_options(postproject-c-smoke PRIVATE -Wall -Wextra -Wpedantic -Werror)
endif()

foreach(consumer IN ITEMS postproject-c-transaction-lifetime
                          postproject-cpp-transaction-lifetime)
    if(MSVC)
        target_compile_options(${consumer} PRIVATE /W4 /WX)
        target_compile_definitions(${consumer} PRIVATE _CRT_SECURE_NO_WARNINGS)
    else()
        target_compile_options(${consumer} PRIVATE -Wall -Wextra -Wpedantic -Werror)
    endif()
endforeach()

enable_testing()
add_test(NAME postproject-c-transaction-lifetime
    COMMAND postproject-c-transaction-lifetime "${CMAKE_CURRENT_BINARY_DIR}/c-lifetime.pproj")
add_test(NAME postproject-cpp-transaction-lifetime
    COMMAND postproject-cpp-transaction-lifetime "${CMAKE_CURRENT_BINARY_DIR}/cpp-lifetime.pproj")
add_test(NAME postproject-cpp-smoke COMMAND postproject-cpp-smoke "${CMAKE_CURRENT_BINARY_DIR}/cpp-smoke.pproj")
add_test(
    NAME postproject-cpp-no-exceptions
    COMMAND postproject-cpp-no-exceptions "${CMAKE_CURRENT_BINARY_DIR}/cpp-no-exceptions.pproj"
)
add_test(
    NAME postproject-c-smoke
    COMMAND postproject-c-smoke
        "${CMAKE_CURRENT_BINARY_DIR}/c-smoke.pproj"
        "${CMAKE_CURRENT_BINARY_DIR}"
)
