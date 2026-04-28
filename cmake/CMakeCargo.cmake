# cargo_build(NAME <crate_name> [SHARED])
#
# Invokes `cargo build` as a custom command and creates CMake IMPORTED library
# targets for the resulting artifacts. Always produces a static library;
# optionally also produces a shared library when SHARED is specified.
#
# The crate must declare `crate-type = ["staticlib", "cdylib"]` in its
# Cargo.toml for both output types to be produced.
#
# Created targets (using underscored crate name):
#   <crate_name>                —  STATIC IMPORTED library
#   <crate_name>_shared         —  SHARED IMPORTED library  (only if SHARED)
#
# Auxiliary targets (used internally for dependency ordering):
#   <crate_name>_target         —  custom target that drives the cargo build
#                                  (depends on the static lib output)
#   <crate_name>_shared_target  —  custom target that depends on the shared
#                                  lib output (only if SHARED)
#
# The caller controls Rust compiler flags via CMAKE_Rust_FLAGS (passed as
# RUSTFLAGS) and the build profile via CMAKE_BUILD_TYPE.
function(cargo_build)
  cmake_parse_arguments(CARGO "SHARED" "NAME" "" ${ARGN})
  string(REPLACE "-" "_" LIB_NAME ${CARGO_NAME})

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

  # ── 3. Compute expected output paths ───────────────────────────────────
  #
  # Cargo places artifacts at:
  #   <CARGO_TARGET_DIR>/<triple>/<profile>/lib<name>.a      (staticlib)
  #   <CARGO_TARGET_DIR>/<triple>/<profile>/lib<name>.so     (cdylib)
  #
  # We list all expected outputs so the custom command's OUTPUT list is
  # complete — this lets the build system know which files the command
  # produces and avoids unnecessary re-runs.

  set(LIB_FILE "${CARGO_TARGET_DIR}/${LIB_TARGET}/${LIB_BUILD_TYPE}/${CMAKE_STATIC_LIBRARY_PREFIX}${LIB_NAME}${CMAKE_STATIC_LIBRARY_SUFFIX}")
  set(LIB_OUTPUTS ${LIB_FILE})
  if(CARGO_SHARED)
    set(LIB_FILE_SHARED "${CARGO_TARGET_DIR}/${LIB_TARGET}/${LIB_BUILD_TYPE}/${CMAKE_SHARED_LIBRARY_PREFIX}${LIB_NAME}${CMAKE_SHARED_LIBRARY_SUFFIX}")
    list(APPEND LIB_OUTPUTS ${LIB_FILE_SHARED})
    # On Windows, shared libraries also produce an import library (.lib).
    if(WIN32)
      set(LIB_FILE_SHARED_IMPLIB "${LIB_FILE_SHARED}${CMAKE_IMPORT_LIBRARY_SUFFIX}")
      list(APPEND LIB_OUTPUTS ${LIB_FILE_SHARED_IMPLIB})
    endif()
  endif()

  # ── 4. Assemble the cargo command line ─────────────────────────────────

  set(CARGO_ARGS "build")
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

  # ── 5. Register the custom command and targets ─────────────────────────
  #
  # The custom command runs `cargo build` whenever any .rs source file
  # changes. The custom targets provide named handles that other CMake
  # targets can depend on.
  #
  # We then wrap the raw cargo outputs in IMPORTED library targets so the
  # rest of the CMake build can link against them using standard
  # target_link_libraries() calls.

  # Glob all Rust sources so the command re-runs when any of them change.
  file(GLOB_RECURSE LIB_SOURCES "*.rs")
  list(APPEND LIB_SOURCES
    "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
    "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.lock"
  )

  # Set CARGO_TARGET_DIR and RUSTFLAGS in the cargo process environment.
  set(CARGO_ENV_COMMAND ${CMAKE_COMMAND} -E env "CARGO_TARGET_DIR=${CARGO_TARGET_DIR}" "RUSTFLAGS=${CMAKE_Rust_FLAGS}")

  add_custom_command(
    OUTPUT ${LIB_OUTPUTS}
    COMMAND ${CARGO_ENV_COMMAND} ${CARGO_EXECUTABLE} ARGS ${CARGO_ARGS}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    DEPENDS ${LIB_SOURCES}
    COMMENT "running cargo"
  )

  # Static library — always created.
  add_custom_target(${CARGO_NAME}_target ALL DEPENDS ${LIB_FILE})
  add_library(${CARGO_NAME} STATIC IMPORTED GLOBAL)
  add_dependencies(${CARGO_NAME} ${CARGO_NAME}_target)
  set_target_properties(${CARGO_NAME} PROPERTIES IMPORTED_LOCATION ${LIB_FILE})

  # Shared library — only when SHARED was requested.
  if(CARGO_SHARED)
    add_custom_target(${CARGO_NAME}_shared_target ALL DEPENDS ${LIB_FILE_SHARED})
    add_library(${CARGO_NAME}_shared SHARED IMPORTED GLOBAL)
    add_dependencies(${CARGO_NAME}_shared ${CARGO_NAME}_shared_target)
    set_target_properties(${CARGO_NAME}_shared PROPERTIES IMPORTED_LOCATION ${LIB_FILE_SHARED})
    if(WIN32)
      set_target_properties(${CARGO_NAME}_shared PROPERTIES IMPORTED_IMPLIB ${LIB_FILE_SHARED_IMPLIB})
    endif()
  endif()
endfunction()
