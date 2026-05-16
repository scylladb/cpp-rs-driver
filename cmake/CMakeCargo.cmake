# ============================================================================
# Dependency diagram  —  full picture (Linux names for concreteness)
# ============================================================================
#
# This diagram shows how cargo_build() (this file) and the caller
# (scylla-rust-wrapper/CMakeLists.txt) cooperate to produce the final
# library artifacts. Arrows point from dependee to depender (i.e. A → B
# means "B depends on A").
#
# Each crate type (staticlib, cdylib) is built by a separate
# cargo_build() invocation using `cargo rustc --crate-type`. Both
# invocations share the same CARGO_TARGET_DIR, so intermediate Rust
# compilation artifacts are reused when feature flags match.
#
#    ┌──────────────────────────┐     ┌──────────────────────────────┐
#    │ cargo rustc              │     │ cargo rustc                  │
#    │  --crate-type staticlib  │     │  --crate-type cdylib         │
#    │ (add_custom_command)     │     │ (add_custom_command)         │
#    │                          │     │                              │
#    │ OUTPUT:                  │     │ OUTPUT:                      │
#    │  .../<profile>/          │     │  .../<profile>/              │
#    │  libscylla_cpp_driver.a  │     │  libscylla_cpp_driver.so     │
#    └────────────┬─────────────┘     └──────────────┬───────────────┘
#                 │                                  │
#    ┌────────────▼─────────────┐     ┌──────────────▼───────────────┐
#    │ scylla_cpp_driver        │     │ scylla_cpp_driver            │
#    │ _staticlib_target        │     │ _cdylib_target               │
#    │ (custom target, ALL)     │     │ (custom target, ALL)         │
#    └────────────┬─────────────┘     └──────────────┬───────────────┘
#                 │                                  │
#    ┌────────────▼─────────────┐     ┌──────────────▼───────────────┐
#    │ scylla_cpp_driver        │     │ scylla_cpp_driver            │
#    │ _staticlib               │     │ _cdylib                      │
#    │ (STATIC IMPORTED)        │     │ (SHARED IMPORTED)            │
#    │                          │     │                              │
#    │ IMPORTED_LOCATION:       │     │ IMPORTED_LOCATION:           │
#    │  .../<profile>/          │     │  .../<profile>/              │
#    │  libscylla_cpp_driver.a  │     │  libscylla_cpp_driver.so     │
#    └────────────┬─────────────┘     └──────────────┬───────────────┘
#                 │                                  │
#      $<TARGET_FILE:…>                   $<TARGET_FILE:…>
#      (used by create_copy)              (used by create_copy)
#                 │                                  │
#    ┌────────────▼─────────────┐     ┌──────────────▼───────────────┐
#    │ libscylla-cpp-driver     │     │ libscylla-cpp-driver         │
#    │ _static.a_copy           │     │ .so.X.Y.Z_copy              │
#    │ (custom target, ALL)     │     │ (custom target, ALL)         │
#    │                          │     │                              │
#    │ Copies + renames         │     │ Copies + renames             │
#    │ into build/              │     │ into build/                  │
#    └────────────┬─────────────┘     └──────────────┬───────────────┘
#                 │                                  │
#    ┌────────────▼─────────────┐     ┌──────────────▼───────────────┐
#    │ scylla-cpp-driver_static │     │ scylla-cpp-driver            │
#    │ (STATIC IMPORTED)        │     │ (SHARED IMPORTED)            │
#    │                          │     │                              │
#    │ IMPORTED_LOCATION:       │     │ IMPORTED_LOCATION:           │
#    │  build/libscylla-cpp-    │     │  build/libscylla-cpp-        │
#    │  driver_static.a         │     │  driver.so.X.Y.Z            │
#    └────────────┬─────────────┘     │                              │
#                 │                   │ + symlinks:                  │
#                 │                   │  .so.X  →  .so.X.Y.Z       │
#                 │                   │  .so    →  .so.X.Y.Z       │
#                 │                   └──────────────┬───────────────┘
#                 │                                  │
#                 ▼                                  ▼
#          tests, examples, install
#
# ============================================================================
# Target layers  —  summary
# ============================================================================
#
# Layer  What                             Purpose
# ─────  ────────────────────────────     ────────────────────────────────────
#  (1)   cargo rustc --crate-type …      Separate custom command per crate
#         (custom commands)               type. Shares intermediate artifacts
#                                         via a common CARGO_TARGET_DIR.
#
#  (2)   _staticlib_target /             Custom targets that depend on the raw
#         _cdylib_target                  artifacts; give the build system a
#         (custom targets, ALL)           named handle to order on.
#
#  (3)   scylla_cpp_driver_staticlib /   IMPORTED targets wrapping the raw cargo
#         scylla_cpp_driver_cdylib       output paths. Exist solely so that
#         (IMPORTED libraries)            create_copy can use $<TARGET_FILE:…>
#                                         generator expressions to get the path.
#
#  (4)   _copy targets                    Copy + rename from Cargo's underscore
#         (custom targets, ALL)           naming (libscylla_cpp_driver.so) to
#                                         conventional C library naming
#                                         (libscylla-cpp-driver.so.X.Y.Z) in
#                                         the build root.
#
#  (5)   scylla-cpp-driver_static /       Final IMPORTED targets pointing at the
#         scylla-cpp-driver               renamed copies. These are what the
#         (IMPORTED libraries)            rest of the build system links against.
#
# ============================================================================

# cargo_build(NAME <crate_name> CRATE_TYPE <type> [FEATURES <feature> ...])
#
# Invokes `cargo rustc --crate-type <type>` as a custom command and creates
# an IMPORTED library target wrapping the resulting artifact. The caller is
# responsible for copying/renaming the artifact and creating a final IMPORTED
# library target that the rest of the build system links against.
#
# Each crate type should be built by a separate cargo_build() call. Multiple
# calls with the same NAME share CARGO_TARGET_DIR, so Rust intermediate
# compilation artifacts are reused when feature flags match.
#
# The crate must declare the requested crate-type in its Cargo.toml.
#
# Created targets:
#   <crate_name>_<crate_type>          —  IMPORTED library (STATIC or SHARED)
#   <crate_name>_<crate_type>_target   —  custom target driving the cargo build
#
# The caller controls Rust compiler flags via CMAKE_Rust_FLAGS (passed as
# RUSTFLAGS) and the build profile via CMAKE_BUILD_TYPE. Additional
# environment variables for the cargo process can be passed via ENV.
function(cargo_build)
  cmake_parse_arguments(CARGO "" "NAME;CRATE_TYPE" "FEATURES;ENV" ${ARGN})
  string(REPLACE "-" "_" LIB_NAME ${CARGO_NAME})

  if(NOT CARGO_CRATE_TYPE)
    message(FATAL_ERROR "cargo_build: CRATE_TYPE is required")
  endif()

  # ── 1. Resolve the Rust target triple ──────────────────────────────────
  #
  # Cargo needs an explicit --target <triple> argument. Map the current
  # CMake platform/architecture to the corresponding Rust triple.

  set(CARGO_TARGET_DIR ${CMAKE_CURRENT_BINARY_DIR})

  if(WIN32)
    if(CMAKE_SIZEOF_VOID_P EQUAL 8)
      set(LIB_TARGET "x86_64-pc-windows-msvc")
    else()
      set(LIB_TARGET "i686-pc-windows-msvc")
    endif()
  elseif(CMAKE_SYSTEM_NAME STREQUAL Darwin)
    if(CMAKE_SYSTEM_PROCESSOR STREQUAL "arm64")
      set(LIB_TARGET "aarch64-apple-darwin")
    else()
      set(LIB_TARGET "${CMAKE_SYSTEM_PROCESSOR}-apple-darwin")
    endif()
  elseif(CMAKE_SYSTEM_NAME STREQUAL Linux)
    if(CMAKE_SIZEOF_VOID_P EQUAL 8)
      set(LIB_TARGET "${CMAKE_SYSTEM_PROCESSOR}-unknown-linux-gnu")
    else()
      set(LIB_TARGET "i686-unknown-linux-gnu")
    endif()
  else()
    message(FATAL_ERROR "${CMAKE_SYSTEM_NAME} is unknown system")
  endif()

  # ── 2. Map CMAKE_BUILD_TYPE to a Cargo profile ────────────────────────
  #
  # Cargo profile names don't match CMake's build types 1:1, so we
  # translate. The profile name also determines the output subdirectory
  # under target/<triple>/<profile>/.

  if(NOT CMAKE_BUILD_TYPE)
    set(LIB_BUILD_TYPE "debug")
  elseif(${CMAKE_BUILD_TYPE} STREQUAL "Release")
    set(LIB_BUILD_TYPE "release")
  elseif(${CMAKE_BUILD_TYPE} STREQUAL "RelWithDebInfo")
    set(LIB_BUILD_TYPE "relwithdebinfo")
  else()
    set(LIB_BUILD_TYPE "debug")
  endif()

  # ── 3. Compute expected output path ────────────────────────────────────
  #
  # Cargo places artifacts at:
  #   <CARGO_TARGET_DIR>/<triple>/<profile>/lib<name>.a      (staticlib)
  #   <CARGO_TARGET_DIR>/<triple>/<profile>/lib<name>.so     (cdylib)

  set(LIB_OUTPUT_DIR "${CARGO_TARGET_DIR}/${LIB_TARGET}/${LIB_BUILD_TYPE}")

  if(CARGO_CRATE_TYPE STREQUAL "staticlib")
    set(LIB_FILE "${LIB_OUTPUT_DIR}/${CMAKE_STATIC_LIBRARY_PREFIX}${LIB_NAME}${CMAKE_STATIC_LIBRARY_SUFFIX}")
    set(LIB_OUTPUTS ${LIB_FILE})
  elseif(CARGO_CRATE_TYPE STREQUAL "cdylib")
    set(LIB_FILE "${LIB_OUTPUT_DIR}/${CMAKE_SHARED_LIBRARY_PREFIX}${LIB_NAME}${CMAKE_SHARED_LIBRARY_SUFFIX}")
    set(LIB_OUTPUTS ${LIB_FILE})
    # On Windows, shared libraries also produce an import library (.lib).
    if(WIN32)
      set(LIB_FILE_IMPLIB "${LIB_FILE}${CMAKE_IMPORT_LIBRARY_SUFFIX}")
      list(APPEND LIB_OUTPUTS ${LIB_FILE_IMPLIB})
    endif()
  else()
    message(FATAL_ERROR "cargo_build: unsupported CRATE_TYPE '${CARGO_CRATE_TYPE}' (expected 'staticlib' or 'cdylib')")
  endif()

  # ── 4. Assemble the cargo command line ─────────────────────────────────
  #
  # We use `cargo rustc --crate-type <type>` instead of `cargo build` so
  # that each invocation produces only the requested artifact type.
  # Multiple invocations share the same CARGO_TARGET_DIR, so intermediate
  # Rust compilation artifacts are reused when feature flags match.


  set(CARGO_ARGS "rustc" "--crate-type" "${CARGO_CRATE_TYPE}")
  list(APPEND CARGO_ARGS "--target" ${LIB_TARGET})
  if (CMAKE_VERBOSE_MAKEFILE)
    list(APPEND CARGO_ARGS "--verbose")
  endif()

  if(${LIB_BUILD_TYPE} STREQUAL "release")
    list(APPEND CARGO_ARGS "--release")
  elseif(${LIB_BUILD_TYPE} STREQUAL "relwithdebinfo")
    list(APPEND CARGO_ARGS "--profile" "relwithdebinfo")
  elseif(${LIB_BUILD_TYPE} STREQUAL "debug")
    list(APPEND CARGO_ARGS "--profile" "dev")
  endif()

  if(CARGO_FEATURES)
    list(JOIN CARGO_FEATURES "," CARGO_FEATURES_STR)
    list(APPEND CARGO_ARGS "--features" "${CARGO_FEATURES_STR}")
  endif()

  # ── 5. Register the custom command and target ──────────────────────────
  #
  # The custom command runs `cargo rustc` whenever any .rs source file
  # changes. The custom target provides a named handle that other CMake
  # targets can depend on.
  #
  # We then wrap the raw cargo output in an IMPORTED library target so
  # the caller can use $<TARGET_FILE:…> to obtain the artifact path.

  # Glob all Rust sources so the command re-runs when any of them change.
  file(GLOB_RECURSE LIB_SOURCES "*.rs")
  list(APPEND LIB_SOURCES
    "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
    "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.lock"
  )

  # Set CARGO_TARGET_DIR and RUSTFLAGS in the cargo process environment.
  set(CARGO_ENV_COMMAND ${CMAKE_COMMAND} -E env
    "CARGO_TARGET_DIR=${CARGO_TARGET_DIR}"
    "RUSTFLAGS=${CMAKE_Rust_FLAGS}"
    ${CARGO_ENV})

  add_custom_command(
    OUTPUT ${LIB_OUTPUTS}
    COMMAND ${CARGO_ENV_COMMAND} ${CARGO_EXECUTABLE} ARGS ${CARGO_ARGS}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    DEPENDS ${LIB_SOURCES}
    COMMENT "running cargo rustc --crate-type ${CARGO_CRATE_TYPE}"
  )

  add_custom_target(${CARGO_NAME}_${CARGO_CRATE_TYPE}_target ALL DEPENDS ${LIB_OUTPUTS})

  if(CARGO_CRATE_TYPE STREQUAL "staticlib")
    add_library(${CARGO_NAME}_${CARGO_CRATE_TYPE} STATIC IMPORTED GLOBAL)
  else()
    add_library(${CARGO_NAME}_${CARGO_CRATE_TYPE} SHARED IMPORTED GLOBAL)
  endif()
  add_dependencies(${CARGO_NAME}_${CARGO_CRATE_TYPE} ${CARGO_NAME}_${CARGO_CRATE_TYPE}_target)
  set_target_properties(${CARGO_NAME}_${CARGO_CRATE_TYPE} PROPERTIES IMPORTED_LOCATION ${LIB_FILE})

  if(CARGO_CRATE_TYPE STREQUAL "cdylib" AND WIN32)
    set_target_properties(${CARGO_NAME}_${CARGO_CRATE_TYPE} PROPERTIES IMPORTED_IMPLIB ${LIB_FILE_IMPLIB})
  endif()
endfunction()
