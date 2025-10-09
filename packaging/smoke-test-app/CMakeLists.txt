cmake_minimum_required(VERSION 3.15)
project(scylla_cpp_driver_smoke_app LANGUAGES C)

# Derive driver version information from pkg-config metadata when the
# scylla-cpp-driver dev package is available. This keeps the smoke-app
# aligned with the installed driver packages instead of relying on
# building the driver from the same CMake project.
get_filename_component(CPP_DRIVER_ROOT "${CMAKE_CURRENT_LIST_DIR}/.." ABSOLUTE)
get_filename_component(CPP_DRIVER_ROOT "${CPP_DRIVER_ROOT}/.." ABSOLUTE)

find_package(PkgConfig REQUIRED)
pkg_check_modules(SCYLLA_CPP_DRIVER REQUIRED IMPORTED_TARGET scylla-cpp-driver)

option(SCYLLA_SMOKE_BUILD_STATIC
  "Build statically linked smoke test executable" OFF)

set(_pkg_config_use_static_libs_saved OFF)
if(DEFINED PKG_CONFIG_USE_STATIC_LIBS)
  set(_pkg_config_use_static_libs_saved "${PKG_CONFIG_USE_STATIC_LIBS}")
endif()
set(PKG_CONFIG_USE_STATIC_LIBS ON)
pkg_check_modules(SCYLLA_CPP_DRIVER_STATIC IMPORTED_TARGET scylla-cpp-driver_static)
if(_pkg_config_use_static_libs_saved)
  set(PKG_CONFIG_USE_STATIC_LIBS "${_pkg_config_use_static_libs_saved}")
else()
  unset(PKG_CONFIG_USE_STATIC_LIBS)
endif()
unset(_pkg_config_use_static_libs_saved)

set(PROJECT_VERSION_STRING "")
if(SCYLLA_CPP_DRIVER_VERSION)
  set(PROJECT_VERSION_STRING "${SCYLLA_CPP_DRIVER_VERSION}")
  string(REPLACE "-" ";" _version_parts "${PROJECT_VERSION_STRING}")
  list(GET _version_parts 0 _numeric_version)
  list(LENGTH _version_parts _parts_length)
  if(_parts_length GREATER 1)
    list(REMOVE_AT _version_parts 0)
    list(JOIN _version_parts "-" PROJECT_VERSION_SUFFIX)
  else()
    set(PROJECT_VERSION_SUFFIX "")
  endif()

  string(REPLACE "." ";" _numeric_split "${_numeric_version}")
  list(LENGTH _numeric_split _numeric_length)
  if(_numeric_length GREATER 0)
    list(GET _numeric_split 0 PROJECT_VERSION_MAJOR)
  else()
    set(PROJECT_VERSION_MAJOR "")
  endif()
  if(_numeric_length GREATER 1)
    list(GET _numeric_split 1 PROJECT_VERSION_MINOR)
  else()
    set(PROJECT_VERSION_MINOR "")
  endif()
  if(_numeric_length GREATER 2)
    list(GET _numeric_split 2 PROJECT_VERSION_PATCH)
  else()
    set(PROJECT_VERSION_PATCH "")
  endif()
endif()

if(PROJECT_VERSION_STRING STREQUAL "")
  # Fallback to parsing cassandra.h when pkg-config metadata does not provide
  # version information (should be rare, but keeps backwards compatibility).
  set(CASS_INCLUDE_DIR "")
  foreach(_candidate_dir IN LISTS SCYLLA_CPP_DRIVER_INCLUDE_DIRS)
    if(EXISTS "${_candidate_dir}/cassandra.h")
      set(CASS_INCLUDE_DIR "${_candidate_dir}")
      break()
    endif()
  endforeach()
  if(CASS_INCLUDE_DIR STREQUAL "")
    set(CASS_INCLUDE_DIR "${CPP_DRIVER_ROOT}/include")
  endif()

  file(STRINGS "${CASS_INCLUDE_DIR}/cassandra.h" _VERSION_PARTS
    REGEX "^#define[ \t]+CASS_VERSION_(MAJOR|MINOR|PATCH|SUFFIX)[ \t]+([0-9]+|\"([^\"]+)\")$")

  foreach(part MAJOR MINOR PATCH SUFFIX)
    string(REGEX MATCH "CASS_VERSION_${part}[ \t]+([0-9]+|\"([^\"]+)\")"
      PROJECT_VERSION_${part} "${_VERSION_PARTS}")
    if(PROJECT_VERSION_${part})
      string(REGEX REPLACE "CASS_VERSION_${part}[ \t]+([0-9]+|\"([^\"]+)\")" "\\1"
        PROJECT_VERSION_${part} "${PROJECT_VERSION_${part}}")
    endif()
  endforeach()

  if(NOT PROJECT_VERSION_MAJOR OR NOT PROJECT_VERSION_MINOR)
    message(FATAL_ERROR "Unable to extract driver version from metadata or cassandra.h")
  endif()

  set(PROJECT_VERSION_STRING
    "${PROJECT_VERSION_MAJOR}.${PROJECT_VERSION_MINOR}")
  if(NOT PROJECT_VERSION_PATCH STREQUAL "")
    set(PROJECT_VERSION_STRING
      "${PROJECT_VERSION_STRING}.${PROJECT_VERSION_PATCH}")
  endif()
  if(NOT PROJECT_VERSION_SUFFIX STREQUAL "")
    string(REPLACE "\"" "" PROJECT_VERSION_SUFFIX ${PROJECT_VERSION_SUFFIX})
    set(PROJECT_VERSION_STRING
      "${PROJECT_VERSION_STRING}-${PROJECT_VERSION_SUFFIX}")
  endif()
endif()

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
if(PROJECT_VERSION_SUFFIX)
  string(REPLACE "\"" "" PROJECT_VERSION_SUFFIX ${PROJECT_VERSION_SUFFIX})
  if(PROJECT_VERSION_PATCH)
    set(PROJECT_VERSION_STRING
      "${PROJECT_VERSION_MAJOR}.${PROJECT_VERSION_MINOR}.${PROJECT_VERSION_PATCH}-${PROJECT_VERSION_SUFFIX}")
  else()
    set(PROJECT_VERSION_STRING
      "${PROJECT_VERSION_MAJOR}.${PROJECT_VERSION_MINOR}-${PROJECT_VERSION_SUFFIX}")
  endif()
elseif(PROJECT_VERSION_STRING STREQUAL "")
  set(PROJECT_VERSION_STRING
    "${PROJECT_VERSION_MAJOR}.${PROJECT_VERSION_MINOR}")
  if(NOT PROJECT_VERSION_PATCH STREQUAL "")
    set(PROJECT_VERSION_STRING
      "${PROJECT_VERSION_STRING}.${PROJECT_VERSION_PATCH}")
  endif()
endif()

add_executable(scylla-cpp-driver-smoke-test
  src/smoke_test.c)

set(_smoke_link_target "PkgConfig::SCYLLA_CPP_DRIVER")
set(_smoke_use_static TRUE)
set(_smoke_extra_link_libraries "")
if(WIN32)
  set(_smoke_use_static FALSE)
else()
  if(APPLE)
    set(_smoke_shared_patterns ".dylib" ".dylib.*")
  else()
    set(_smoke_shared_patterns ".so" ".so.*")
  endif()
  foreach(_smoke_dir IN LISTS SCYLLA_CPP_DRIVER_LIBRARY_DIRS)
    foreach(_smoke_lib IN LISTS SCYLLA_CPP_DRIVER_LIBRARIES)
      foreach(_smoke_pattern IN LISTS _smoke_shared_patterns)
        file(GLOB _smoke_shared_candidates
          "${_smoke_dir}/lib${_smoke_lib}${_smoke_pattern}")
        if(_smoke_shared_candidates)
          set(_smoke_use_static FALSE)
          break()
        endif()
      endforeach()
      if(NOT _smoke_use_static)
        break()
      endif()
    endforeach()
    if(NOT _smoke_use_static)
      break()
    endif()
  endforeach()
endif()
if(_smoke_use_static)
  if(SCYLLA_CPP_DRIVER_STATIC_FOUND AND TARGET PkgConfig::SCYLLA_CPP_DRIVER_STATIC)
    message(STATUS
      "scylla-cpp-driver shared library not found; linking smoke test statically")
    set(_smoke_link_target "PkgConfig::SCYLLA_CPP_DRIVER_STATIC")
    if(NOT WIN32)
      list(APPEND _smoke_extra_link_libraries m)
    endif()
  else()
    message(FATAL_ERROR
      "scylla-cpp-driver shared library not found and static package metadata unavailable; cannot link smoke test")
  endif()
endif()
unset(_smoke_dir)
unset(_smoke_lib)
unset(_smoke_pattern)
unset(_smoke_shared_candidates)
unset(_smoke_shared_patterns)
unset(_smoke_use_static)

target_link_libraries(scylla-cpp-driver-smoke-test PRIVATE
  ${_smoke_link_target}
  ${_smoke_extra_link_libraries})
unset(_smoke_extra_link_libraries)
unset(_smoke_link_target)

if(SCYLLA_SMOKE_BUILD_STATIC)
  if(NOT SCYLLA_CPP_DRIVER_STATIC_FOUND)
    message(FATAL_ERROR
      "Static scylla-cpp-driver pkg-config metadata required to build static smoke test")
  endif()

  add_executable(scylla-cpp-driver-smoke-test-static
    src/smoke_test.c)
  set_target_properties(scylla-cpp-driver-smoke-test-static PROPERTIES
    OUTPUT_NAME "scylla-cpp-driver-smoke-test-static")
  if(NOT PKG_CONFIG_EXECUTABLE)
    message(FATAL_ERROR "pkg-config executable not available for static linkage")
  endif()
  execute_process(
    COMMAND ${PKG_CONFIG_EXECUTABLE} --libs --static scylla-cpp-driver_static
    OUTPUT_VARIABLE _smoke_static_libs_output
    RESULT_VARIABLE _smoke_static_libs_result
    OUTPUT_STRIP_TRAILING_WHITESPACE)
  if(NOT _smoke_static_libs_result EQUAL 0 OR _smoke_static_libs_output STREQUAL "")
    message(FATAL_ERROR
      "Unable to resolve static link flags from scylla-cpp-driver_static pkg-config metadata")
  endif()
  separate_arguments(_smoke_static_link_libraries UNIX_COMMAND "${_smoke_static_libs_output}")
  if(NOT WIN32)
    list(APPEND _smoke_static_link_libraries m)
  endif()
  target_link_libraries(scylla-cpp-driver-smoke-test-static PRIVATE
    ${_smoke_static_link_libraries})
  unset(_smoke_static_libs_output)
  unset(_smoke_static_link_libraries)
endif()

set(SMOKE_APP_COMPONENT_NAME "")
set(SMOKE_APP_COMPONENT_ARGS "")
if(APPLE)
  set(SMOKE_APP_COMPONENT_NAME "scylla_cpp_driver_smoke_app")
  set(SMOKE_APP_COMPONENT_ARGS COMPONENT ${SMOKE_APP_COMPONENT_NAME})
endif()

set(_smoke_install_targets scylla-cpp-driver-smoke-test)
if(TARGET scylla-cpp-driver-smoke-test-static)
  list(APPEND _smoke_install_targets scylla-cpp-driver-smoke-test-static)
endif()

install(TARGETS
  ${_smoke_install_targets}
  DESTINATION bin
  ${SMOKE_APP_COMPONENT_ARGS})
unset(_smoke_install_targets)

#------------------------
# Packaging (CPack)
#------------------------

if(NOT DEFINED CPACK_PACKAGE_NAME)
  set(CPACK_PACKAGE_NAME "scylla-cpp-driver-smoke-app")
endif()
set(_SMOKE_APP_PACKAGE_BASENAME "${CPACK_PACKAGE_NAME}")
set(CPACK_PACKAGE_VENDOR "ScyllaDB")
set(CPACK_PACKAGE_CONTACT "ScyllaDB <info@scylladb.com>")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY
  "Smoke test application for scylla-cpp-driver binary packages")
set(CPACK_PACKAGE_HOMEPAGE_URL "https://github.com/scylladb/cpp-rs-driver")
set(CPACK_PACKAGE_VERSION ${PROJECT_VERSION_STRING})
set(CPACK_PACKAGE_VERSION_MAJOR ${PROJECT_VERSION_MAJOR})
set(CPACK_PACKAGE_VERSION_MINOR ${PROJECT_VERSION_MINOR})
set(CPACK_PACKAGE_VERSION_PATCH ${PROJECT_VERSION_PATCH})
set(CPACK_VERBATIM_VARIABLES ON)
set(CPACK_MONOLITHIC_INSTALL ON)

if(WIN32)
  set(CPACK_GENERATOR "WIX")
  set(CPACK_PACKAGE_INSTALL_DIRECTORY "ScyllaDB\\Scylla CPP Driver Smoke Test")
  set(CPACK_WIX_UPGRADE_GUID "180C9F7E-8D90-40F1-A91E-9DE5DB451A80")
elseif(APPLE)
  set(CPACK_GENERATOR "productbuild;DragNDrop")
  configure_file(
    "${CPP_DRIVER_ROOT}/LICENSE"
    "${CMAKE_CURRENT_BINARY_DIR}/LICENSE.txt"
    COPYONLY)
  set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_CURRENT_BINARY_DIR}/LICENSE.txt")
  set(CPACK_PACKAGE_FILE_NAME
    "${_SMOKE_APP_PACKAGE_BASENAME}-${PROJECT_VERSION_STRING}-macos")
  set(CPACK_PRODUCTBUILD_IDENTIFIER "com.scylladb.cpp-rs-driver.smoke-app")
  set(CPACK_PRODUCTBUILD_SIGNING_IDENTITY "")
  if(SMOKE_APP_COMPONENT_NAME)
    include(CPackComponent)
    set(CPACK_MONOLITHIC_INSTALL OFF)
    set(CPACK_COMPONENTS_ALL ${SMOKE_APP_COMPONENT_NAME})
    cpack_add_component(${SMOKE_APP_COMPONENT_NAME}
      DISPLAY_NAME "Smoke Test Binary"
      DESCRIPTION "CLI smoke test for installed driver packages"
      REQUIRED)
    set(CPACK_PRODUCTBUILD_COMPONENT_INSTALL ON)
  endif()
else()
  set(CPACK_GENERATOR "DEB;RPM")
  set(CPACK_RESOURCE_FILE_LICENSE "${CPP_DRIVER_ROOT}/LICENSE")
  set(CPACK_PACKAGE_FILE_NAME
    "${_SMOKE_APP_PACKAGE_BASENAME}-${PROJECT_VERSION_STRING}-${CMAKE_SYSTEM_NAME}")
  set(CPACK_DEBIAN_FILE_NAME DEB-DEFAULT)
  set(CPACK_DEBIAN_PACKAGE_MAINTAINER "ScyllaDB")
  set(CPACK_DEBIAN_PACKAGE_SECTION "database")
  set(CPACK_DEBIAN_PACKAGE_HOMEPAGE ${CPACK_PACKAGE_HOMEPAGE_URL})
  set(CPACK_DEBIAN_PACKAGE_SHLIBDEPS ON)
  set(CPACK_RPM_PACKAGE_LICENSE "Apache-2.0")
  set(CPACK_RPM_PACKAGE_URL ${CPACK_PACKAGE_HOMEPAGE_URL})
  set(CPACK_RPM_PACKAGE_RELEASE 1)
  set(CPACK_RPM_PACKAGE_GROUP "Applications/Databases")
endif()

include(CPack)
unset(_SMOKE_APP_PACKAGE_BASENAME)
