cmake_minimum_required(VERSION 3.21)

project(postproject_c_example LANGUAGES C)

find_package(PostProject 0.5 REQUIRED CONFIG)

add_executable(postproject-c-example main.c)
target_compile_features(postproject-c-example PRIVATE c_std_11)
target_link_libraries(postproject-c-example PRIVATE PostProject::postproject)

if(WIN32)
    add_custom_command(
        TARGET postproject-c-example
        POST_BUILD
        COMMAND "${CMAKE_COMMAND}" -E copy_if_different
            "$<TARGET_FILE:PostProject::postproject>"
            "$<TARGET_FILE_DIR:postproject-c-example>"
    )
endif()

enable_testing()
add_test(
    NAME postproject-c-example
    COMMAND postproject-c-example
        "${CMAKE_CURRENT_BINARY_DIR}/c-example.pproj"
        "${CMAKE_CURRENT_SOURCE_DIR}/../fixtures/sample-media.dat"
)
