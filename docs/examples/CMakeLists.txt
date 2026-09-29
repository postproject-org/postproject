cmake_minimum_required(VERSION 3.21)

# Builds and runs every non-Rust listing included in the documentation. Configure it
# only against an installed PostProject prefix; it never invokes Cargo. The
# Rust listings run as the postproject-doc-examples workspace test.
project(postproject_doc_examples LANGUAGES C CXX)

find_package(PostProject 0.5 REQUIRED CONFIG)
find_package(Python3 3.11 COMPONENTS Interpreter)
find_program(POSTPROJECT_EXECUTABLE postproject)
find_program(BASH_EXECUTABLE bash)
find_program(JQ_EXECUTABLE jq)

set(POSTPROJECT_MEDIA_FIXTURE
    "${CMAKE_CURRENT_SOURCE_DIR}/../../examples/fixtures/sample-media.dat"
    CACHE FILEPATH "Media file imported by the guide examples")
set(POSTPROJECT_PYTHON_SOURCE
    "${CMAKE_CURRENT_SOURCE_DIR}/../../python/src"
    CACHE PATH "Directory containing the postproject Python package")

# Every source in a surface directory is one example program with its own test.
file(GLOB c_examples CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/c/*.c")
file(GLOB cpp_examples CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/cpp/*.cpp")
file(GLOB python_examples CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/python/*.py")
file(GLOB cli_examples CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/cli/*.sh")

enable_testing()

# Each example gets a fresh work directory prepared by a setup fixture.
function(add_example_test name)
    set(work_dir "${CMAKE_CURRENT_BINARY_DIR}/work-${name}")
    add_test(
        NAME "${name}-prepare"
        COMMAND "${CMAKE_COMMAND}"
            "-DWORK_DIR=${work_dir}"
            "-DMEDIA_FIXTURE=${POSTPROJECT_MEDIA_FIXTURE}"
            -P "${CMAKE_CURRENT_SOURCE_DIR}/prepare-workdir.cmake"
    )
    set_tests_properties("${name}-prepare" PROPERTIES FIXTURES_SETUP "${name}-work")
    add_test(NAME "${name}" COMMAND ${ARGN} "${work_dir}")
    set_tests_properties("${name}" PROPERTIES FIXTURES_REQUIRED "${name}-work")
endfunction()

function(add_native_example surface source standard)
    get_filename_component(stem "${source}" NAME_WE)
    set(target "postproject-${surface}-${stem}")
    add_executable("${target}" "${source}")
    target_compile_features("${target}" PRIVATE "${standard}")
    target_link_libraries("${target}" PRIVATE PostProject::postproject)
    if(MSVC)
        target_compile_options("${target}" PRIVATE /W4 /WX)
        target_compile_definitions("${target}" PRIVATE _CRT_SECURE_NO_WARNINGS)
    else()
        target_compile_options("${target}" PRIVATE -Wall -Wextra -Wpedantic -Werror)
    endif()
    if(WIN32)
        add_custom_command(
            TARGET "${target}"
            POST_BUILD
            COMMAND "${CMAKE_COMMAND}" -E copy_if_different
                "$<TARGET_FILE:PostProject::postproject>"
                "$<TARGET_FILE_DIR:${target}>"
        )
    endif()
    add_example_test("${surface}-${stem}" "$<TARGET_FILE:${target}>")
endfunction()

foreach(source IN LISTS c_examples)
    add_native_example(c "${source}" c_std_11)
endforeach()
foreach(source IN LISTS cpp_examples)
    add_native_example(cpp "${source}" cxx_std_17)
endforeach()

if(Python3_FOUND)
    foreach(source IN LISTS python_examples)
        get_filename_component(stem "${source}" NAME_WE)
        add_example_test("python-${stem}" "${Python3_EXECUTABLE}" "${source}")
        set_tests_properties(
            "python-${stem}"
            PROPERTIES
                ENVIRONMENT_MODIFICATION
                    "PYTHONPATH=path_list_prepend:${POSTPROJECT_PYTHON_SOURCE};POSTPROJECT_LIBRARY=set:$<TARGET_FILE:PostProject::postproject>"
        )
    endforeach()
else()
    message(WARNING "Python 3.11 or newer not found; skipping the Python examples")
endif()

if(NOT WIN32 AND POSTPROJECT_EXECUTABLE AND BASH_EXECUTABLE AND JQ_EXECUTABLE)
    get_filename_component(postproject_bin "${POSTPROJECT_EXECUTABLE}" DIRECTORY)
    foreach(source IN LISTS cli_examples)
        get_filename_component(stem "${source}" NAME_WE)
        add_example_test("cli-${stem}" "${BASH_EXECUTABLE}" "${source}")
        set_tests_properties(
            "cli-${stem}"
            PROPERTIES ENVIRONMENT_MODIFICATION "PATH=path_list_prepend:${postproject_bin}"
        )
    endforeach()
else()
    message(WARNING "postproject, bash, or jq not found; skipping the CLI examples")
endif()
