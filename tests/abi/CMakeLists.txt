cmake_minimum_required(VERSION 3.21)

project(postproject_native_consumers LANGUAGES C CXX)

find_package(PostProject 0.5 REQUIRED CONFIG)

# Check the installed header's type distinction with the consumer compiler.
include(CheckCSourceCompiles)
include(CMakePushCheckState)
cmake_push_check_state(RESET)
set(CMAKE_REQUIRED_LIBRARIES PostProject::postproject)
check_c_source_compiles("#include <postproject/postproject.h>
int main(void) { pp_production_id_t id = {{0}};
  return pp_production_id_format(id, 0, 0); }" POSTPROJECT_TYPED_ID_COMPILES)
if(NOT POSTPROJECT_TYPED_ID_COMPILES)
    message(FATAL_ERROR "Installed production ID example must compile")
endif()
check_c_source_compiles("#include <postproject/postproject.h>
int main(void) { pp_uuid_t id = {{0}};
  return pp_production_id_format(id, 0, 0); }" POSTPROJECT_RAW_ID_COMPILES)
if(POSTPROJECT_RAW_ID_COMPILES)
    message(FATAL_ERROR "Generic UUID must not substitute for production ID")
endif()
check_c_source_compiles("#include <postproject/postproject.h>
int main(void) { pp_revision_id_t id = {{0}};
  return pp_production_revision_events_page(0, id, 1, 0, 0, 0); }"
    POSTPROJECT_REVISION_ID_COMPILES)
if(NOT POSTPROJECT_REVISION_ID_COMPILES)
    message(FATAL_ERROR "Installed revision ID call must compile")
endif()
check_c_source_compiles("#include <postproject/postproject.h>
int main(void) { pp_transaction_id_t id = {{0}};
  return pp_production_revision_events_page(0, id, 1, 0, 0, 0); }"
    POSTPROJECT_TRANSACTION_AS_REVISION_COMPILES)
if(POSTPROJECT_TRANSACTION_AS_REVISION_COMPILES)
    message(FATAL_ERROR "Transaction identity must not substitute for revision identity")
endif()
check_c_source_compiles("#include <postproject/postproject.h>
int main(void) { pp_asset_id_t id = {{0}};
  return pp_asset_id_format(id, 0, 0); }" POSTPROJECT_ASSET_ID_COMPILES)
if(NOT POSTPROJECT_ASSET_ID_COMPILES)
    message(FATAL_ERROR "Installed asset ID example must compile")
endif()
check_c_source_compiles("#include <postproject/postproject.h>
int main(void) { pp_production_id_t id = {{0}};
  return pp_asset_id_format(id, 0, 0); }" POSTPROJECT_PRODUCTION_AS_ASSET_COMPILES)
if(POSTPROJECT_PRODUCTION_AS_ASSET_COMPILES)
    message(FATAL_ERROR "Production identity must not substitute for asset identity")
endif()
check_c_source_compiles("#include <postproject/postproject.h>
int main(void) { pp_asset_id_t id = {{0}};
  return pp_production_asset(0, id, 0, 0); }" POSTPROJECT_ASSET_READ_COMPILES)
if(NOT POSTPROJECT_ASSET_READ_COMPILES)
    message(FATAL_ERROR "Installed typed asset read must compile")
endif()
check_c_source_compiles("#include <postproject/postproject.h>
int main(void) { pp_uuid_t id = {{0}};
  return pp_production_asset(0, id, 0, 0); }" POSTPROJECT_RAW_ASSET_READ_COMPILES)
if(POSTPROJECT_RAW_ASSET_READ_COMPILES)
    message(FATAL_ERROR "Generic UUID must not substitute in an asset read")
endif()
check_c_source_compiles("#include <postproject/postproject.h>
int main(void) { pp_media_root_id_t id = {{0}};
  return pp_transaction_set_media_root_enabled(0, id, 0, 0); }"
    POSTPROJECT_ROOT_ID_COMPILES)
if(NOT POSTPROJECT_ROOT_ID_COMPILES)
    message(FATAL_ERROR "Installed typed root mutation must compile")
endif()
check_c_source_compiles("#include <postproject/postproject.h>
int main(void) { pp_asset_id_t id = {{0}};
  return pp_transaction_set_media_root_enabled(0, id, 0, 0); }"
    POSTPROJECT_ASSET_AS_ROOT_COMPILES)
if(POSTPROJECT_ASSET_AS_ROOT_COMPILES)
    message(FATAL_ERROR "Asset identity must not substitute for root identity")
endif()
cmake_pop_check_state()

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

add_executable(postproject-c-read-sessions c_read_sessions.c)
target_compile_features(postproject-c-read-sessions PRIVATE c_std_11)
target_link_libraries(postproject-c-read-sessions PRIVATE PostProject::postproject)

add_executable(postproject-c-identities c_identities.c)
target_compile_features(postproject-c-identities PRIVATE c_std_11)
target_link_libraries(postproject-c-identities PRIVATE PostProject::postproject)

add_executable(postproject-cpp-identities cpp_identities.cpp)
target_compile_features(postproject-cpp-identities PRIVATE cxx_std_17)
target_link_libraries(postproject-cpp-identities PRIVATE PostProject::postproject)

add_executable(postproject-cpp-transaction-lifetime cpp_transaction_lifetime.cpp)
target_compile_features(postproject-cpp-transaction-lifetime PRIVATE cxx_std_17)
target_link_libraries(postproject-cpp-transaction-lifetime PRIVATE PostProject::postproject)

if(WIN32)
    foreach(consumer IN ITEMS postproject-c-smoke postproject-cpp-smoke
                             postproject-cpp-no-exceptions
                             postproject-c-transaction-lifetime
                             postproject-c-read-sessions
                             postproject-c-identities
                             postproject-cpp-identities
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
                          postproject-c-read-sessions
                          postproject-c-identities
                          postproject-cpp-identities
                          postproject-cpp-transaction-lifetime)
    if(MSVC)
        target_compile_options(${consumer} PRIVATE /W4 /WX)
        target_compile_definitions(${consumer} PRIVATE _CRT_SECURE_NO_WARNINGS)
    else()
        target_compile_options(${consumer} PRIVATE -Wall -Wextra -Wpedantic -Werror)
    endif()
endforeach()

enable_testing()
add_test(NAME postproject-c-identities COMMAND postproject-c-identities)
add_test(NAME postproject-cpp-identities COMMAND postproject-cpp-identities)
add_test(NAME postproject-c-read-sessions
    COMMAND postproject-c-read-sessions "${CMAKE_CURRENT_BINARY_DIR}/c-read-sessions.pproj")
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
