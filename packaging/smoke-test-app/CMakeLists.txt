cmake_minimum_required(VERSION 3.15)
project(scylla_cpp_driver_smoke_app LANGUAGES C)

# Derive driver version information from the root header so the smoke-app
# package stays in lockstep with the driver packages.
get_filename_component(CPP_DRIVER_ROOT "${CMAKE_CURRENT_LIST_DIR}/.." ABSOLUTE)
get_filename_component(CPP_DRIVER_ROOT "${CPP_DRIVER_ROOT}/.." ABSOLUTE)
set(CASS_INCLUDE_DIR "${CPP_DRIVER_ROOT}/include")

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
  message(FATAL_ERROR "Unable to extract driver version from cassandra.h")
endif()

set(PROJECT_VERSION_STRING
  "${PROJECT_VERSION_MAJOR}.${PROJECT_VERSION_MINOR}")
if(NOT PROJECT_VERSION_PATCH STREQUAL "")
  set(PROJECT_VERSION_STRING
    "${PROJECT_VERSION_STRING}.${PROJECT_VERSION_PATCH}")
endif()
if(NOT PROJECT_VERSION_SUFFIX STREQUAL "")
  string(REPLACE "\"" ""
    PROJECT_VERSION_SUFFIX ${PROJECT_VERSION_SUFFIX})
  set(PROJECT_VERSION_STRING
    "${PROJECT_VERSION_STRING}-${PROJECT_VERSION_SUFFIX}")
endif()

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

find_package(PkgConfig)
if(PKG_CONFIG_FOUND)
  pkg_check_modules(SCYLLA_CPP_DRIVER scylla-cpp-driver)
endif()

set(DRIVER_INCLUDE_DIRS "")
set(DRIVER_LIBRARY_DIRS "")
set(DRIVER_LIBRARIES "")
set(DRIVER_COMPILE_OPTIONS "")
set(DRIVER_LINK_OPTIONS "")

if(PKG_CONFIG_FOUND AND SCYLLA_CPP_DRIVER_FOUND)
  list(APPEND DRIVER_INCLUDE_DIRS ${SCYLLA_CPP_DRIVER_INCLUDE_DIRS})
  list(APPEND DRIVER_LIBRARY_DIRS ${SCYLLA_CPP_DRIVER_LIBRARY_DIRS})
  list(APPEND DRIVER_LIBRARIES ${SCYLLA_CPP_DRIVER_LIBRARIES})
  list(APPEND DRIVER_COMPILE_OPTIONS ${SCYLLA_CPP_DRIVER_CFLAGS_OTHER})
  list(APPEND DRIVER_LINK_OPTIONS ${SCYLLA_CPP_DRIVER_LDFLAGS_OTHER})
endif()

if(NOT PKG_CONFIG_FOUND OR NOT SCYLLA_CPP_DRIVER_FOUND)
  if(NOT DEFINED SCYLLA_CPP_DRIVER_ROOT AND DEFINED ENV{SCYLLA_CPP_DRIVER_ROOT})
    set(SCYLLA_CPP_DRIVER_ROOT "$ENV{SCYLLA_CPP_DRIVER_ROOT}")
  endif()
  if(NOT DEFINED SCYLLA_CPP_DRIVER_ROOT)
    if(WIN32)
      set(SCYLLA_CPP_DRIVER_ROOT "C:/Program Files/ScyllaDB/Scylla CPP Driver")
    elseif(APPLE)
      set(SCYLLA_CPP_DRIVER_ROOT "/usr/local")
    else()
      set(SCYLLA_CPP_DRIVER_ROOT "/usr")
    endif()
  endif()

  find_path(SCYLLA_CPP_DRIVER_INCLUDE_DIR cassandra.h
    PATHS
      "${SCYLLA_CPP_DRIVER_ROOT}/include"
    PATH_SUFFIXES
      ""
      "cassandra"
      "x86_64-linux-gnu"
      "x86_64-linux-gnu/cassandra")

  find_library(SCYLLA_CPP_DRIVER_LIBRARY
    NAMES scylla-cpp-driver
    PATHS
      "${SCYLLA_CPP_DRIVER_ROOT}/lib"
      "${SCYLLA_CPP_DRIVER_ROOT}/lib64"
      "${SCYLLA_CPP_DRIVER_ROOT}/lib/x86_64-linux-gnu"
      "${SCYLLA_CPP_DRIVER_ROOT}/lib64/x86_64-linux-gnu")

  if(NOT SCYLLA_CPP_DRIVER_INCLUDE_DIR OR NOT SCYLLA_CPP_DRIVER_LIBRARY)
    message(FATAL_ERROR
      "Unable to locate scylla-cpp-driver installation. Set SCYLLA_CPP_DRIVER_ROOT "
      "or install pkg-config so the package can be discovered automatically.")
  endif()

  set(SCYLLA_CPP_DRIVER_FOUND TRUE)
  set(DRIVER_INCLUDE_DIRS ${SCYLLA_CPP_DRIVER_INCLUDE_DIR})
  get_filename_component(_driver_library_dir ${SCYLLA_CPP_DRIVER_LIBRARY} DIRECTORY)
  set(DRIVER_LIBRARY_DIRS ${_driver_library_dir})
  set(DRIVER_LIBRARIES ${SCYLLA_CPP_DRIVER_LIBRARY})
endif()

add_executable(scylla-cpp-driver-smoke-test
  src/smoke_test.c)

if(DRIVER_INCLUDE_DIRS)
  target_include_directories(scylla-cpp-driver-smoke-test PRIVATE
    ${DRIVER_INCLUDE_DIRS})
endif()

if(DRIVER_LIBRARY_DIRS)
  target_link_directories(scylla-cpp-driver-smoke-test PRIVATE
    ${DRIVER_LIBRARY_DIRS})
endif()

if(DRIVER_LIBRARIES)
  target_link_libraries(scylla-cpp-driver-smoke-test PRIVATE
    ${DRIVER_LIBRARIES})
endif()

if(DRIVER_COMPILE_OPTIONS)
  target_compile_options(scylla-cpp-driver-smoke-test PRIVATE
    ${DRIVER_COMPILE_OPTIONS})
endif()

if(DRIVER_LINK_OPTIONS)
  target_link_options(scylla-cpp-driver-smoke-test PRIVATE
    ${DRIVER_LINK_OPTIONS})
endif()

install(TARGETS scylla-cpp-driver-smoke-test DESTINATION bin)

#------------------------
# Packaging (CPack)
#------------------------

set(CPACK_PACKAGE_NAME "scylla-cpp-driver-smoke-app")
set(CPACK_PACKAGE_VENDOR "ScyllaDB")
set(CPACK_PACKAGE_CONTACT "ScyllaDB <info@scylladb.com>")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY
  "Smoke test application for scylla-cpp-driver binary packages")
set(CPACK_PACKAGE_HOMEPAGE_URL "https://github.com/scylladb/cpp-rust-driver")
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
  set(CPACK_RESOURCE_FILE_LICENSE "${CPP_DRIVER_ROOT}/LICENSE")
  set(CPACK_PACKAGE_FILE_NAME
    "scylla-cpp-driver-smoke-app-${PROJECT_VERSION_STRING}-macos")
  set(CPACK_PRODUCTBUILD_IDENTIFIER "com.scylladb.cpp-rust-driver.smoke-app")
  set(CPACK_PRODUCTBUILD_SIGNING_IDENTITY "")
else()
  set(CPACK_GENERATOR "DEB;RPM")
  set(CPACK_RESOURCE_FILE_LICENSE "${CPP_DRIVER_ROOT}/LICENSE")
  set(CPACK_PACKAGE_FILE_NAME
    "scylla-cpp-driver-smoke-app-${PROJECT_VERSION_STRING}-${CMAKE_SYSTEM_NAME}")
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
