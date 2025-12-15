cmake_minimum_required(VERSION 3.15)
project(cppdriverv2 C CXX)

set(CASS_ROOT_DIR ${CMAKE_CURRENT_SOURCE_DIR})
set(CASS_SRC_DIR "${CASS_ROOT_DIR}/src")
set(CASS_INCLUDE_DIR "${CASS_ROOT_DIR}/include")

# Ensure functions/modules are available
list(APPEND CMAKE_MODULE_PATH ${CASS_ROOT_DIR}/cmake)

set(CMAKE_EXPORT_COMPILE_COMMANDS TRUE)
set_property(GLOBAL PROPERTY USE_FOLDERS ON)

enable_language(Rust)
include(CMakeCargo)

#---------------
# Policies
#---------------

if(POLICY CMP0074)
  cmake_policy(SET CMP0074 NEW)
endif()

if (POLICY CMP0054)
  cmake_policy(SET CMP0054 NEW)
endif()

#---------------
# Options
#---------------

option(CASS_BUILD_EXAMPLES "Build examples" OFF)
option(CASS_BUILD_INTEGRATION_TESTS "Build integration tests" OFF)
option(CASS_BUILD_SHARED "Build shared library" ON)
option(CASS_BUILD_STATIC "Build static library" ON)
option(CASS_BUILD_TESTS "Build tests" OFF)
option(CASS_BUILD_UNIT_TESTS "Build unit tests" OFF)
option(CASS_DEBUG_CUSTOM_ALLOC "Debug custom allocator" OFF)
option(CASS_INSTALL_HEADER "Install header file" ON)
option(CASS_INSTALL_HEADER_IN_SUBDIR "Install header file under 'include/cassandra'" OFF)
option(CASS_INSTALL_PKG_CONFIG "Install pkg-config file(s)" ON)
option(CASS_MULTICORE_COMPILATION "Enable multicore compilation" ON)
option(CASS_USE_BOOST_ATOMIC "Use Boost atomics library" OFF)
option(CASS_USE_LIBSSH2 "Use libssh2 for integration tests" OFF)
option(CASS_USE_OPENSSL "Use OpenSSL" ON)
option(CASS_USE_STATIC_LIBS "Link static libraries when building executables" OFF)
option(CASS_USE_STD_ATOMIC "Use std::atomic library" ON)
option(CASS_USE_TIMERFD "Use timerfd (Linux only)" ON)
option(CASS_USE_LIBUV "Use libuv" OFF)

set(CASS_CPP_STANDARD "11" CACHE STRING "C++ standard (11, 14, 17, etc.)")

# Component metadata used for macOS productbuild packages. Other platforms
# ignore these values, so keep the defaults empty by default.
set(SCYLLA_DRIVER_COMPONENT_NAME "scylla_cpp_driver")
set(SCYLLA_DRIVER_COMPONENT_ARGS COMPONENT ${SCYLLA_DRIVER_COMPONENT_NAME})

# Define component names for runtime and dev packages
# These must be set before add_subdirectory() calls so install commands can use them
set(SCYLLA_DRIVER_RUNTIME_COMPONENT_NAME "${SCYLLA_DRIVER_COMPONENT_NAME}")
set(SCYLLA_DRIVER_DEV_COMPONENT_NAME "${SCYLLA_DRIVER_COMPONENT_NAME}-dev")
string(TOUPPER "${SCYLLA_DRIVER_RUNTIME_COMPONENT_NAME}" SCYLLA_DRIVER_RUNTIME_COMPONENT_NAME_UPPER)
string(TOUPPER "${SCYLLA_DRIVER_DEV_COMPONENT_NAME}" SCYLLA_DRIVER_DEV_COMPONENT_NAME_UPPER)

if(CASS_BUILD_SHARED)
  set(BUILD_SHARED_LIBS ON)
endif()
# Handle testing dependencies
if(CASS_BUILD_TESTS)
  # Enable integration and unit tests
  set(CASS_BUILD_INTEGRATION_TESTS ON)
  set(CASS_BUILD_UNIT_TESTS ON)
endif()

if(CASS_BUILD_INTEGRATION_TESTS OR CASS_BUILD_UNIT_TESTS)
  set(CASS_USE_OPENSSL ON) # Required for tests
  set(CASS_USE_LIBUV ON)
endif()

if(CASS_BUILD_EXAMPLES)
  set(CASS_USE_LIBUV ON) # Some examples require libuv, e.g. "callbacks".
endif()

# Determine which driver target should be used as a dependency
set(PROJECT_LIB_NAME_TARGET scylla-cpp-driver)
if(CASS_USE_STATIC_LIBS OR
   (WIN32 AND (CASS_BUILD_INTEGRATION_TESTS OR CASS_BUILD_UNIT_TESTS)))
  set(CASS_USE_STATIC_LIBS ON) # Not all driver internals are exported for test executable (e.g. CASS_EXPORT)
  set(CASS_BUILD_STATIC ON)
  set(PROJECT_LIB_NAME_TARGET scylla-cpp-driver_static)
endif()

# Ensure the driver is configured to build
if(NOT CASS_BUILD_SHARED AND NOT CASS_BUILD_STATIC)
  message(FATAL_ERROR "Driver is not Configured to Build: Ensure shared and/or static library is enabled")
endif()

if(CASS_DEBUG_CUSTOM_ALLOC AND CASS_USE_STATIC_LIBS)
  message(WARNING "Debugging the custom allocator while static linking the library can cause your application to fail")
endif()

#------------------------
# Dependencies
#------------------------

include(Dependencies)
include(ClangFormat)

#------------------------
# Project Version
#------------------------

file(STRINGS "${CASS_INCLUDE_DIR}/cassandra.h" _VERSION_PARTS
  REGEX "^#define[ \t]+CASS_VERSION_(MAJOR|MINOR|PATCH|SUFFIX)[ \t]+([0-9]+|\"([^\"]+)\")$")

foreach(part MAJOR MINOR PATCH SUFFIX)
  string(REGEX MATCH "CASS_VERSION_${part}[ \t]+([0-9]+|\"([^\"]+)\")"
    PROJECT_VERSION_${part} ${_VERSION_PARTS})
  # Extract version numbers
  if (PROJECT_VERSION_${part})
    string(REGEX REPLACE "CASS_VERSION_${part}[ \t]+([0-9]+|\"([^\"]+)\")" "\\1"
      PROJECT_VERSION_${part} ${PROJECT_VERSION_${part}})
  endif()
endforeach()

# Verify version parts
if(NOT PROJECT_VERSION_MAJOR AND NOT PROJECT_VERSION_MINOR)
  message(FATAL_ERROR "Unable to retrieve driver version from ${version_header_file}")
endif()

set(PROJECT_VERSION_STRING
  ${PROJECT_VERSION_MAJOR}.${PROJECT_VERSION_MINOR})
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

message(STATUS "Driver version: ${PROJECT_VERSION_STRING}")

#------------------------
# Determine atomic implementation
#------------------------

# Determine if std::atomic can be used for GCC, Clang, or MSVC
if("${CMAKE_CXX_COMPILER_ID}" STREQUAL "GNU")
  if(CMAKE_COMPILER_IS_GNUCC OR CMAKE_COMPILER_IS_GNUCXX)
    # Version determined from: https://gcc.gnu.org/wiki/Atomic/GCCMM
    if(CMAKE_CXX_COMPILER_VERSION VERSION_EQUAL "4.7" OR
        CMAKE_CXX_COMPILER_VERSION VERSION_GREATER "4.7")
      set(CASS_USE_STD_ATOMIC ON)
    endif()
  endif()
elseif("${CMAKE_CXX_COMPILER_ID}" STREQUAL "Clang" OR 
       "${CMAKE_CXX_COMPILER_ID}" STREQUAL "AppleClang")
  # Version determined from: http://clang.llvm.org/cxx_status.html
  # 3.2 includes the full C++11 memory model, but 3.1 had atomic
  # support.
  if(CMAKE_CXX_COMPILER_VERSION VERSION_EQUAL "3.1" OR
      CMAKE_CXX_COMPILER_VERSION VERSION_GREATER "3.1")
    set(CASS_USE_STD_ATOMIC ON)
  endif()
elseif("${CMAKE_CXX_COMPILER_ID}" STREQUAL "MSVC")
  # Version determined from https://msdn.microsoft.com/en-us/library/hh874894
  # VS2012+/VS 11.0+/WindowsSDK v8.0+
  if(MSVC_VERSION GREATER 1700 OR
      MSVC_VERSION EQUAL 1700)
    set(CASS_USE_STD_ATOMIC ON)
  endif()
endif()

if(CASS_USE_BOOST_ATOMIC)
  message(STATUS "Using boost::atomic implementation for atomic operations")
elseif(CASS_USE_STD_ATOMIC)
  message(STATUS "Using std::atomic implementation for atomic operations")
endif()

#------------------------
# Top-level compiler flags
#------------------------

set (CMAKE_CXX_STANDARD ${CASS_CPP_STANDARD})

if("${CMAKE_CXX_COMPILER_ID}" STREQUAL "Clang" OR
   "${CMAKE_CXX_COMPILER_ID}" STREQUAL "AppleClang" OR
   "${CMAKE_CXX_COMPILER_ID}" STREQUAL "GNU")

  # OpenSSL is deprecated on later versions of Mac OS X. The long-term solution
  # is to provide a CommonCryto implementation.
  if (APPLE AND CASS_USE_OPENSSL)
    set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -Wno-deprecated-declarations")
  endif()

  if("${CMAKE_CXX_COMPILER_ID}" STREQUAL "Clang" OR
     "${CMAKE_CXX_COMPILER_ID}" STREQUAL "AppleClang") 
    # Clang/Intel specific compiler options
    # I disabled long-long warning because boost generates about 50 such warnings
    set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -Wall -pedantic -Wextra -Wno-long-long -Wno-unused-parameter")
    set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -Wno-variadic-macros -Wno-zero-length-array")
    set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -Wno-unused-local-typedef -Wno-unknown-warning-option")
  else() 
    # GCC specific compiler options
    # I disabled long-long warning because boost generates about 50 such warnings
    set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -Wall -pedantic -Wextra -Wno-long-long -Wno-unused-parameter -Wno-variadic-macros")

    if(CMAKE_CXX_COMPILER_VERSION VERSION_EQUAL "4.8" OR
        CMAKE_CXX_COMPILER_VERSION VERSION_GREATER "4.8")
      set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -Wno-unused-local-typedefs")
    endif()
  endif()
elseif("${CMAKE_CXX_COMPILER_ID}" STREQUAL "MSVC")
  add_definitions(/we4800)

  # Determine if multicore compilation should be enabled
  if(CASS_MULTICORE_COMPILATION)
    # Default multicore compilation with effective processors (see https://msdn.microsoft.com/en-us/library/bb385193.aspx)
    add_definitions("/MP")
  endif()

  # On Visual C++ -pedantic flag is not used,
  # -fPIC is not used on Windows platform (all DLLs are
  # relocable), -Wall generates about 30k stupid warnings
  # that can hide useful ones.
  # Create specific warning disable compiler flags
  # TODO(mpenick): Fix these "possible loss of data" warnings
  add_definitions(/wd4244)
  add_definitions(/wd4267)

  # Add preprocessor definitions for proper compilation
  add_definitions(-D_CRT_SECURE_NO_WARNINGS)  # Remove warnings for not using safe functions (TODO: Fix codebase to be more secure for Visual Studio)
  add_definitions(-DNOMINMAX)                 # Does not define min/max macros
  add_definitions(-D_SILENCE_TR1_NAMESPACE_DEPRECATION_WARNING) # Remove warnings for TR1 deprecation (Visual Studio 15 2017); caused by sparsehash
else()
  message(FATAL_ERROR "Unsupported compiler: ${CMAKE_CXX_COMPILER_ID}")
endif()

#------------------------
# Subdirectories
#------------------------

add_subdirectory(scylla-rust-wrapper)

add_subdirectory(src)

if(CASS_BUILD_EXAMPLES)
  add_subdirectory(examples)
endif()

if(CASS_BUILD_INTEGRATION_TESTS OR CASS_BUILD_TESTS)
  add_subdirectory(tests)
endif()

#------------------------
# Packaging (CPack)
#------------------------

set(_CPACK_PACKAGE_NAME "scylla-cpp-driver")
set(CPACK_PACKAGE_NAME ${_CPACK_PACKAGE_NAME})
set(CPACK_PACKAGE_VENDOR "ScyllaDB")
set(CPACK_PACKAGE_CONTACT "ScyllaDB <info@scylladb.com>")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY
  "ScyllaDB C++ driver backed by the Rust core driver")
set(CPACK_PACKAGE_HOMEPAGE_URL "https://github.com/scylladb/cpp-rs-driver")
set(CPACK_PACKAGE_VERSION ${PROJECT_VERSION_STRING})
set(CPACK_PACKAGE_VERSION_MAJOR ${PROJECT_VERSION_MAJOR})
set(CPACK_PACKAGE_VERSION_MINOR ${PROJECT_VERSION_MINOR})
set(CPACK_PACKAGE_VERSION_PATCH ${PROJECT_VERSION_PATCH})
if(WIN32)
  # Prevent drive letters from bleeding into the staging path when CPack creates
  # the temporary install tree on Windows.
  set(CPACK_PACKAGING_INSTALL_PREFIX "/")
else()
  set(CPACK_PACKAGING_INSTALL_PREFIX "${CMAKE_INSTALL_PREFIX}")
endif()
set(CPACK_MONOLITHIC_INSTALL ON)
set(CPACK_VERBATIM_VARIABLES ON)

# Normalize package output names so generated artifacts share a consistent
# `scylla_cpp_driver_<version>_<platform>.{deb,rpm,pkg,msi}` pattern.
set(CPACK_MONOLITHIC_INSTALL OFF)

set(SCYLLA_DRIVER_ARTIFACT_BASE_NAME ${SCYLLA_DRIVER_COMPONENT_NAME})
set(_scylla_driver_system_processor "${CMAKE_SYSTEM_PROCESSOR}")

set(SCYLLA_DRIVER_DEB_ARCH "${_scylla_driver_system_processor}")
set(SCYLLA_DRIVER_RPM_ARCH "${_scylla_driver_system_processor}")
set(SCYLLA_DRIVER_WIN_ARCH "${_scylla_driver_system_processor}")

if(_scylla_driver_system_processor MATCHES "^(x86_64|AMD64)$")
  set(SCYLLA_DRIVER_DEB_ARCH "amd64")
  set(SCYLLA_DRIVER_RPM_ARCH "x86_64")
  set(SCYLLA_DRIVER_WIN_ARCH "x64")
elseif(_scylla_driver_system_processor MATCHES "^(aarch64|ARM64)$")
  set(SCYLLA_DRIVER_DEB_ARCH "arm64")
  set(SCYLLA_DRIVER_RPM_ARCH "aarch64")
  set(SCYLLA_DRIVER_WIN_ARCH "arm64")
endif()

# Fall back to raw processor strings when no mapping exists.
if(NOT SCYLLA_DRIVER_DEB_ARCH)
  set(SCYLLA_DRIVER_DEB_ARCH "${_scylla_driver_system_processor}")
endif()
if(NOT SCYLLA_DRIVER_RPM_ARCH)
  set(SCYLLA_DRIVER_RPM_ARCH "${_scylla_driver_system_processor}")
endif()
if(NOT SCYLLA_DRIVER_WIN_ARCH)
  set(SCYLLA_DRIVER_WIN_ARCH "${_scylla_driver_system_processor}")
endif()

# Provide a default artifact name that individual generators can refine.
set(CPACK_PACKAGE_FILE_NAME
  "${SCYLLA_DRIVER_ARTIFACT_BASE_NAME}_${PROJECT_VERSION_STRING}")

# Set the list of components for CPack
set(CPACK_COMPONENTS_ALL ${SCYLLA_DRIVER_RUNTIME_COMPONENT_NAME} ${SCYLLA_DRIVER_DEV_COMPONENT_NAME})

# Ensure CPack emits one artifact per component instead of merging them into a
# single bundle.
set(CPACK_COMPONENTS_GROUPING IGNORE)

include(CPackComponent)

# Runtime component: shared libraries only
cpack_add_component(${SCYLLA_DRIVER_RUNTIME_COMPONENT_NAME}
  DISPLAY_NAME "Scylla C++ Driver Runtime"
  DESCRIPTION "Shared libraries for the Scylla C++ Driver"
  REQUIRED)

# Dev component: headers, static libraries, pkg-config files, and development symlinks
cpack_add_component(${SCYLLA_DRIVER_DEV_COMPONENT_NAME}
  DISPLAY_NAME "Scylla C++ Driver Development"
  DESCRIPTION "Headers, static libraries, and development files for the Scylla C++ Driver")

if(WIN32)
  set(CPACK_GENERATOR "WIX")
  set(CPACK_PACKAGE_INSTALL_DIRECTORY "ScyllaDB\\Scylla CPP Driver")
  # Stable GUID keeps WiX upgrades working between versions
  set(CPACK_WIX_UPGRADE_GUID "3E09F4F4-1C4A-4B8B-973D-0CBA64D5E78F")
  set(CPACK_PACKAGE_FILE_NAME
    "${SCYLLA_DRIVER_ARTIFACT_BASE_NAME}_${PROJECT_VERSION_STRING}_${SCYLLA_DRIVER_WIN_ARCH}")
elseif(APPLE)
  configure_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/LICENSE"
    "${CMAKE_CURRENT_BINARY_DIR}/LICENSE.txt"
    COPYONLY)
  set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_CURRENT_BINARY_DIR}/LICENSE.txt")
  set(CPACK_GENERATOR "productbuild;DragNDrop")
  set(CPACK_DMG_COMPONENT_INSTALL ON)
  set(CPACK_PACKAGE_FILE_NAME
    "${SCYLLA_DRIVER_ARTIFACT_BASE_NAME}_${PROJECT_VERSION_STRING}_macos")
  set(CPACK_PRODUCTBUILD_IDENTIFIER "com.scylladb.cpp-rs-driver")
  set(CPACK_PRODUCTBUILD_SIGNING_IDENTITY "")
  set(CPACK_PRODUCTBUILD_COMPONENT_INSTALL ON)

else()
  set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_CURRENT_SOURCE_DIR}/LICENSE")
  set(CPACK_GENERATOR "DEB;RPM")
  set(CPACK_DEBIAN_PACKAGE_ARCHITECTURE "${SCYLLA_DRIVER_DEB_ARCH}")

  set(CPACK_DEBIAN_${SCYLLA_DRIVER_RUNTIME_COMPONENT_NAME_UPPER}_FILE_NAME
    "${SCYLLA_DRIVER_RUNTIME_COMPONENT_NAME}_${PROJECT_VERSION_STRING}_${SCYLLA_DRIVER_DEB_ARCH}.deb")
  set(CPACK_DEBIAN_${SCYLLA_DRIVER_DEV_COMPONENT_NAME_UPPER}_FILE_NAME
    "${SCYLLA_DRIVER_DEV_COMPONENT_NAME}_${PROJECT_VERSION_STRING}_${SCYLLA_DRIVER_DEB_ARCH}.deb")

  # DEB package configuration
  set(CPACK_DEB_COMPONENT_INSTALL ON)
  set(CPACK_DEBIAN_FILE_NAME DEB-DEFAULT)
  set(CPACK_DEBIAN_PACKAGE_MAINTAINER "ScyllaDB")
  set(CPACK_DEBIAN_PACKAGE_SECTION "database")
  set(CPACK_DEBIAN_PACKAGE_HOMEPAGE ${CPACK_PACKAGE_HOMEPAGE_URL})
  set(CPACK_DEBIAN_PACKAGE_SHLIBDEPS ON)

  # DEB runtime package
  set(CPACK_DEBIAN_${SCYLLA_DRIVER_RUNTIME_COMPONENT_NAME_UPPER}_PACKAGE_NAME "${_CPACK_PACKAGE_NAME}")
  set(CPACK_DEBIAN_${SCYLLA_DRIVER_RUNTIME_COMPONENT_NAME_UPPER}_PACKAGE_SECTION "libs")

  # DEB dev package
  set(CPACK_DEBIAN_${SCYLLA_DRIVER_DEV_COMPONENT_NAME_UPPER}_PACKAGE_NAME "${_CPACK_PACKAGE_NAME}-dev")
  set(CPACK_DEBIAN_${SCYLLA_DRIVER_DEV_COMPONENT_NAME_UPPER}_PACKAGE_SECTION "libdevel")

  # RPM package configuration
  set(CPACK_RPM_COMPONENT_INSTALL ON)
  set(CPACK_RPM_PACKAGE_LICENSE "Apache-2.0")
  set(CPACK_RPM_PACKAGE_URL ${CPACK_PACKAGE_HOMEPAGE_URL})
  set(CPACK_RPM_PACKAGE_RELEASE 1)
  set(CPACK_RPM_PACKAGE_GROUP "Applications/Databases")
  set(CPACK_RPM_PACKAGE_ARCHITECTURE "${SCYLLA_DRIVER_RPM_ARCH}")

  set(CPACK_RPM_${SCYLLA_DRIVER_RUNTIME_COMPONENT_NAME}_FILE_NAME
    "${SCYLLA_DRIVER_RUNTIME_COMPONENT_NAME}_${PROJECT_VERSION_STRING}_${SCYLLA_DRIVER_RPM_ARCH}.rpm")

  # RPM runtime package
  set(CPACK_RPM_${SCYLLA_DRIVER_RUNTIME_COMPONENT_NAME}_PACKAGE_NAME "${_CPACK_PACKAGE_NAME}")
  set(CPACK_RPM_${SCYLLA_DRIVER_RUNTIME_COMPONENT_NAME}_PACKAGE_GROUP "System Environment/Libraries")

  # RPM dev package (devel suffix is standard for RPM)
  set(CPACK_RPM_${SCYLLA_DRIVER_DEV_COMPONENT_NAME}_PACKAGE_NAME "${_CPACK_PACKAGE_NAME}-devel")
  set(CPACK_RPM_${SCYLLA_DRIVER_DEV_COMPONENT_NAME}_PACKAGE_GROUP "Development/Libraries")
  set(CPACK_RPM_${SCYLLA_DRIVER_DEV_COMPONENT_NAME}_FILE_NAME
    "${SCYLLA_DRIVER_COMPONENT_NAME}-devel_${PROJECT_VERSION_STRING}_${SCYLLA_DRIVER_RPM_ARCH}.rpm")
endif()

include(CPack)
