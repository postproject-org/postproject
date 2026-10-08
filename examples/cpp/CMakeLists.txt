cmake_minimum_required(VERSION 3.21)

project(postproject_cpp_example LANGUAGES CXX)

find_package(PostProject 0.7 REQUIRED CONFIG)

add_executable(postproject-cpp-example main.cpp)
target_compile_features(postproject-cpp-example PRIVATE cxx_std_17)
target_link_libraries(postproject-cpp-example PRIVATE PostProject::postproject)

if(WIN32)
    add_custom_command(
        TARGET postproject-cpp-example
        POST_BUILD
        COMMAND "${CMAKE_COMMAND}" -E copy_if_different
            "$<TARGET_FILE:PostProject::postproject>"
            "$<TARGET_FILE_DIR:postproject-cpp-example>"
    )
endif()

enable_testing()
add_test(
    NAME postproject-cpp-example
    COMMAND postproject-cpp-example
        "${CMAKE_CURRENT_BINARY_DIR}/cpp-example.pproj"
        "${CMAKE_CURRENT_SOURCE_DIR}/../fixtures/sample-media.dat"
)
