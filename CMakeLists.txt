cmake_minimum_required(VERSION 3.21)

project(PostProject VERSION 0.6.0 LANGUAGES NONE)

set(POSTPROJECT_PACKAGE_VERSION "0.6.0-alpha.1")

include(CMakePackageConfigHelpers)
set(CMAKE_INSTALL_LIBDIR "lib" CACHE PATH "Object code libraries")
include(GNUInstallDirs)

set(POSTPROJECT_LIBRARY "" CACHE FILEPATH "Path to the prebuilt PostProject native library")
set(POSTPROJECT_STATIC_LIBRARY "" CACHE FILEPATH "Optional path to the prebuilt static library")
set(POSTPROJECT_RUNTIME_LIBRARY "" CACHE FILEPATH "Windows DLL paired with POSTPROJECT_LIBRARY")
set(POSTPROJECT_CLI_EXECUTABLE "" CACHE FILEPATH "Optional prebuilt PostProject CLI")
if(NOT POSTPROJECT_LIBRARY)
    message(FATAL_ERROR "POSTPROJECT_LIBRARY must name a library built by cargo build --release -p postproject-ffi")
endif()
if(NOT EXISTS "${POSTPROJECT_LIBRARY}")
    message(FATAL_ERROR "POSTPROJECT_LIBRARY does not exist: ${POSTPROJECT_LIBRARY}")
endif()
if(WIN32)
    if(NOT POSTPROJECT_RUNTIME_LIBRARY)
        message(FATAL_ERROR "POSTPROJECT_RUNTIME_LIBRARY must name the PostProject DLL on Windows")
    endif()
    if(NOT EXISTS "${POSTPROJECT_RUNTIME_LIBRARY}")
        message(FATAL_ERROR "POSTPROJECT_RUNTIME_LIBRARY does not exist: ${POSTPROJECT_RUNTIME_LIBRARY}")
    endif()
endif()

get_filename_component(POSTPROJECT_LIBRARY_FILENAME "${POSTPROJECT_LIBRARY}" NAME)
if(POSTPROJECT_RUNTIME_LIBRARY)
    get_filename_component(POSTPROJECT_RUNTIME_LIBRARY_FILENAME "${POSTPROJECT_RUNTIME_LIBRARY}" NAME)
endif()

install(
    FILES
        include/postproject/postproject.h
        include/postproject/postproject.hpp
    DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}/postproject"
)
install(FILES "${POSTPROJECT_LIBRARY}" DESTINATION "${CMAKE_INSTALL_LIBDIR}")
if(POSTPROJECT_RUNTIME_LIBRARY)
    install(FILES "${POSTPROJECT_RUNTIME_LIBRARY}" DESTINATION "${CMAKE_INSTALL_BINDIR}")
endif()
if(POSTPROJECT_STATIC_LIBRARY)
    if(NOT EXISTS "${POSTPROJECT_STATIC_LIBRARY}")
        message(FATAL_ERROR "POSTPROJECT_STATIC_LIBRARY does not exist: ${POSTPROJECT_STATIC_LIBRARY}")
    endif()
    install(FILES "${POSTPROJECT_STATIC_LIBRARY}" DESTINATION "${CMAKE_INSTALL_LIBDIR}")
endif()
if(POSTPROJECT_CLI_EXECUTABLE)
    if(NOT EXISTS "${POSTPROJECT_CLI_EXECUTABLE}")
        message(FATAL_ERROR "POSTPROJECT_CLI_EXECUTABLE does not exist: ${POSTPROJECT_CLI_EXECUTABLE}")
    endif()
    install(PROGRAMS "${POSTPROJECT_CLI_EXECUTABLE}" DESTINATION "${CMAKE_INSTALL_BINDIR}")
endif()
install(
    FILES LICENSE-APACHE LICENSE-MIT README.md CHANGELOG.md STEWARDSHIP.md
    DESTINATION "${CMAKE_INSTALL_DATADIR}/doc/postproject"
)
install(
    FILES examples/c/CMakeLists.txt examples/c/main.c
    DESTINATION "${CMAKE_INSTALL_DATADIR}/doc/postproject/examples/c"
)
install(
    FILES examples/cpp/CMakeLists.txt examples/cpp/main.cpp
    DESTINATION "${CMAKE_INSTALL_DATADIR}/doc/postproject/examples/cpp"
)
install(
    FILES examples/python/quickstart.py
    DESTINATION "${CMAKE_INSTALL_DATADIR}/doc/postproject/examples/python"
)
install(
    FILES examples/fixtures/sample-media.dat
    DESTINATION "${CMAKE_INSTALL_DATADIR}/doc/postproject/examples/fixtures"
)

configure_package_config_file(
    cmake/PostProjectConfig.cmake.in
    "${CMAKE_CURRENT_BINARY_DIR}/PostProjectConfig.cmake"
    INSTALL_DESTINATION "${CMAKE_INSTALL_LIBDIR}/cmake/PostProject"
    PATH_VARS CMAKE_INSTALL_INCLUDEDIR CMAKE_INSTALL_LIBDIR CMAKE_INSTALL_BINDIR
)
write_basic_package_version_file(
    "${CMAKE_CURRENT_BINARY_DIR}/PostProjectConfigVersion.cmake"
    VERSION "${PROJECT_VERSION}"
    COMPATIBILITY SameMajorVersion
)
configure_file(
    pkgconfig/postproject.pc.in
    "${CMAKE_CURRENT_BINARY_DIR}/postproject.pc"
    @ONLY
)

install(
    FILES
        "${CMAKE_CURRENT_BINARY_DIR}/PostProjectConfig.cmake"
        "${CMAKE_CURRENT_BINARY_DIR}/PostProjectConfigVersion.cmake"
    DESTINATION "${CMAKE_INSTALL_LIBDIR}/cmake/PostProject"
)
install(
    FILES "${CMAKE_CURRENT_BINARY_DIR}/postproject.pc"
    DESTINATION "${CMAKE_INSTALL_LIBDIR}/pkgconfig"
)
