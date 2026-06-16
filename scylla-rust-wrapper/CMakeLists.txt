list(APPEND INCLUDE_DIRS ${CASS_INCLUDE_DIR} ${CMAKE_CURRENT_SOURCE_DIR})

#------------------------------
# Targets
#------------------------------

macro(create_symlink source_file symlink_name)
  add_custom_command(
    OUTPUT ${CMAKE_BINARY_DIR}/${symlink_name}
    COMMAND ${CMAKE_COMMAND} -E create_symlink ${source_file} ${CMAKE_BINARY_DIR}/${symlink_name}
  )
  add_custom_target(${symlink_name}_symlink ALL DEPENDS ${CMAKE_BINARY_DIR}/${symlink_name})
endmacro()

macro(create_copy source_file dest_name)
  add_custom_command(
    OUTPUT ${CMAKE_BINARY_DIR}/${dest_name}
    DEPENDS ${source_file}
    COMMAND ${CMAKE_COMMAND} -E copy ${source_file} ${CMAKE_BINARY_DIR}/${dest_name}
  )
  add_custom_target(${dest_name}_copy ALL DEPENDS ${CMAKE_BINARY_DIR}/${dest_name})
endmacro()

if(DEFINED CMAKE_Rust_FLAGS)
  set(CMAKE_Rust_FLAGS "${CMAKE_Rust_FLAGS} --cfg scylla_unstable")
else()
  set(CMAKE_Rust_FLAGS "--cfg scylla_unstable")
endif()

if(CASS_BUILD_INTEGRATION_TESTS OR CASS_BUILD_EXAMPLES)
  set(CMAKE_Rust_FLAGS "${CMAKE_Rust_FLAGS} --cfg cpp_integration_testing")
endif()

# Shared library names and linker flags differ across platforms, so set them up here.
if(APPLE)
  set(INSTALL_NAME_SHARED "libscylladb.${PROJECT_VERSION_STRING}.dylib")
  set(INSTALL_NAME_SHARED_SYMLINK_VERSION "libscylladb.${PROJECT_VERSION_MAJOR}.dylib")
  set(INSTALL_NAME_SHARED_SYMLINK_NO_VERSION "libscylladb.dylib")
  set(CMAKE_Rust_FLAGS_SONAME "-Clink-arg=-Wl,-install_name,@rpath/libscylladb.${PROJECT_VERSION_MAJOR}.dylib -Clink-arg=-Wl,-current_version,${PROJECT_VERSION_STRING} -Clink-arg=-Wl,-compatibility_version,${PROJECT_VERSION_MAJOR}")
  set(CASSANDRA_COMPAT_SHARED_SYMLINK "libcassandra.dylib")
  set(CASSANDRA_COMPAT_STATIC_SYMLINK "libcassandra_static.a")
elseif(WIN32)
  set(INSTALL_NAME_SHARED "scylladb.dll")
  set(INSTALL_NAME_SHARED_IMPLIB "scylladb.lib")
  set(CMAKE_Rust_FLAGS_SONAME "")
  set(CASSANDRA_COMPAT_SHARED_SYMLINK "cassandra.dll")
  set(CASSANDRA_COMPAT_STATIC_SYMLINK "cassandra_static.lib")
else() # Assume Unix-like (e.g. Linux).
  set(INSTALL_NAME_SHARED "libscylladb.so.${PROJECT_VERSION_STRING}")
  set(INSTALL_NAME_SHARED_SYMLINK_VERSION "libscylladb.so.${PROJECT_VERSION_MAJOR}")
  set(INSTALL_NAME_SHARED_SYMLINK_NO_VERSION "libscylladb.so")
  set(CMAKE_Rust_FLAGS_SONAME "-Clink-arg=-Wl,-soname=libscylladb.so.${PROJECT_VERSION_MAJOR}")
  set(CASSANDRA_COMPAT_SHARED_SYMLINK "libcassandra.so")
  set(CASSANDRA_COMPAT_STATIC_SYMLINK "libcassandra_static.a")
endif()

# Static library names are more consistent across platforms, but still differ on Windows.
if(WIN32)
  set(INSTALL_NAME_STATIC "scylladb_static.lib")
else()
  set(INSTALL_NAME_STATIC "libscylladb_static.a")
endif()

# Make rustc pass the SONAME flag to the linker when building the shared library.
if(DEFINED CMAKE_Rust_FLAGS)
  set(CMAKE_Rust_FLAGS "${CMAKE_Rust_FLAGS} ${CMAKE_Rust_FLAGS_SONAME}")
else()
  set(CMAKE_Rust_FLAGS "${CMAKE_Rust_FLAGS_SONAME}")
endif()

if(CASS_BUILD_SHARED)
  cargo_build(NAME scylladb CRATE_TYPE cdylib)
  create_copy($<TARGET_FILE:scylladb_cdylib> ${INSTALL_NAME_SHARED})
  add_library(scylladb SHARED IMPORTED GLOBAL)
  add_dependencies(scylladb ${INSTALL_NAME_SHARED}_copy)
  set_target_properties(scylladb PROPERTIES IMPORTED_LOCATION ${CMAKE_BINARY_DIR}/${INSTALL_NAME_SHARED})
  if(WIN32)
    get_target_property(SCYLLADB_IMPLIB scylladb_cdylib IMPORTED_IMPLIB)
    if(SCYLLADB_IMPLIB)
      create_copy(${SCYLLADB_IMPLIB} ${INSTALL_NAME_SHARED_IMPLIB})
      set_target_properties(scylladb PROPERTIES IMPORTED_IMPLIB ${CMAKE_BINARY_DIR}/${INSTALL_NAME_SHARED_IMPLIB})
    endif()
  endif()
  if(NOT WIN32)
    create_symlink(${INSTALL_NAME_SHARED} ${INSTALL_NAME_SHARED_SYMLINK_VERSION})
    create_symlink(${INSTALL_NAME_SHARED} ${INSTALL_NAME_SHARED_SYMLINK_NO_VERSION})
    if(CASS_INSTALL_CASSANDRA_COMPAT)
      create_symlink(${INSTALL_NAME_SHARED_SYMLINK_NO_VERSION} ${CASSANDRA_COMPAT_SHARED_SYMLINK})
    endif()
  endif()
endif()

if(CASS_BUILD_STATIC)
  cargo_build(NAME scylladb CRATE_TYPE staticlib)
  create_copy($<TARGET_FILE:scylladb_staticlib> ${INSTALL_NAME_STATIC})
  add_library(scylladb_static STATIC IMPORTED GLOBAL)
  add_dependencies(scylladb_static ${INSTALL_NAME_STATIC}_copy)
  set_target_properties(scylladb_static PROPERTIES IMPORTED_LOCATION ${CMAKE_BINARY_DIR}/${INSTALL_NAME_STATIC})
  if(CASS_INSTALL_CASSANDRA_COMPAT AND NOT WIN32)
    create_symlink(${INSTALL_NAME_STATIC} ${CASSANDRA_COMPAT_STATIC_SYMLINK})
  endif()

  # The Rust staticlib contains unresolved references to OpenSSL and system
  # libraries.  Declare them as transitive (INTERFACE) dependencies so that
  # any target linking against scylladb_static automatically pulls
  # in the required symbols in the correct order.
  #
  # NOTE: Some of these libraries (e.g. OpenSSL, ws2_32, crypt32, userenv on
  # Windows) also appear in CASS_LIBS (populated by cmake/Dependencies.cmake).
  # This causes them to appear twice on the linker command line, which is
  # intentional and harmless -- linkers silently ignore duplicate inputs --
  # but it keeps the static target self-contained so it works correctly even
  # when linked outside the integration-test build where CASS_LIBS is set.

  # ## `INTERFACE_LINK_LIBRARIES` on an IMPORTED target
  # For an **IMPORTED** target (which `scylladb_static` is — `add_library(scylladb_static STATIC IMPORTED GLOBAL)`),
  # the `INTERFACE_LINK_LIBRARIES` property means: "when something links against this target,
  # also link against these libraries. This is actually the **correct** property to use here. The distinction:
  # - `LINK_LIBRARIES` — libraries used to build the target itself (irrelevant for IMPORTED targets
  #   since they're pre-built)
  # - `INTERFACE_LINK_LIBRARIES` — libraries that **consumers** of this target must also link against
  #
  # Since `scylladb_static` is a pre-built static archive, it doesn't get "built" by CMake — it's already compiled.
  # But any executable that links against it (like `cassandra-integration-tests`) needs to also link
  # `-lssl -lcrypto -ldl -lm -lpthread` to resolve the symbols that the archive references.
  # That's exactly what `INTERFACE_LINK_LIBRARIES` communicates.
  #
  # The word "INTERFACE" in CMake doesn't mean "public API" in the C++ sense — it means "propagated to consumers."
  # For an IMPORTED static library, ALL transitive dependencies are necessarily interface dependencies because
  # the static archive is opaque to CMake (it can't see inside it to know what symbols are needed).
  #
  # ## Comparison with `Libs.private` in pkg-config
  #
  # This is the CMake equivalent of `Libs.private` in the `.pc` file — both say
  # "consumers need these additional libraries when linking statically."
  # The naming is confusing but semantically correct:
  # - pkg-config: `Libs.private` = "needed by consumers for static linking"
  # - CMake: `INTERFACE_LINK_LIBRARIES` on IMPORTED STATIC = "needed by consumers"
  set(_STATIC_INTERFACE_LIBS "")

  # These variables are used to generate the static library pkg-config file,
  # and are separate from _STATIC_INTERFACE_LIBS because pkg-config requires
  # a different format (e.g. "-lssl -lcrypto" instead of a list of library paths).
  set(scylladb_static_requires_private "")
  set(scylladb_static_libs_private "")

  if(CASS_USE_OPENSSL AND OPENSSL_LIBRARIES)
    list(APPEND _STATIC_INTERFACE_LIBS ${OPENSSL_LIBRARIES})
    list(APPEND scylladb_static_requires_private openssl)
  endif()

  if(WIN32)
    # Windows system libraries required by Rust runtime and its dependencies
    list(APPEND _STATIC_INTERFACE_LIBS ws2_32 bcrypt ntdll userenv crypt32 secur32 ncrypt)
    list(APPEND scylladb_static_libs_private
      "-lws2_32" "-lbcrypt" "-lntdll" "-luserenv" "-lcrypt32" "-lsecur32" "-lncrypt")
  elseif(APPLE)
    list(APPEND _STATIC_INTERFACE_LIBS ${CMAKE_DL_LIBS} m)
    if(CMAKE_DL_LIBS)
      list(APPEND scylladb_static_libs_private "-l${CMAKE_DL_LIBS}")
    endif()
    list(APPEND scylladb_static_libs_private "-lm")
    find_package(Threads)
    if(Threads_FOUND)
      list(APPEND _STATIC_INTERFACE_LIBS Threads::Threads)
      if(CMAKE_THREAD_LIBS_INIT)
        list(APPEND scylladb_static_libs_private "${CMAKE_THREAD_LIBS_INIT}")
      endif()
    endif()
    # CoreFoundation is needed by the iana-time-zone crate on macOS
    find_library(COREFOUNDATION_LIBRARY CoreFoundation)
    if(COREFOUNDATION_LIBRARY)
      list(APPEND _STATIC_INTERFACE_LIBS ${COREFOUNDATION_LIBRARY})
      list(APPEND scylladb_static_libs_private "-framework CoreFoundation")
    endif()
    # SystemConfiguration and Security may also be needed on macOS
    find_library(SYSTEMCONFIGURATION_LIBRARY SystemConfiguration)
    if(SYSTEMCONFIGURATION_LIBRARY)
      list(APPEND _STATIC_INTERFACE_LIBS ${SYSTEMCONFIGURATION_LIBRARY})
      list(APPEND scylladb_static_libs_private "-framework SystemConfiguration")
    endif()
    find_library(SECURITY_LIBRARY Security)
    if(SECURITY_LIBRARY)
      list(APPEND _STATIC_INTERFACE_LIBS ${SECURITY_LIBRARY})
      list(APPEND scylladb_static_libs_private "-framework Security")
    endif()
  else() # non-Windows and non-Apple - assume Unix-like, such as Linux
    list(APPEND _STATIC_INTERFACE_LIBS ${CMAKE_DL_LIBS} m)
    if(CMAKE_DL_LIBS)
      list(APPEND scylladb_static_libs_private "-l${CMAKE_DL_LIBS}")
    endif()
    list(APPEND scylladb_static_libs_private "-lm")
    find_package(Threads)
    if(Threads_FOUND)
      list(APPEND _STATIC_INTERFACE_LIBS Threads::Threads)
      if(CMAKE_THREAD_LIBS_INIT)
        list(APPEND scylladb_static_libs_private "${CMAKE_THREAD_LIBS_INIT}")
      endif()
    endif()
  endif()

  if(_STATIC_INTERFACE_LIBS)
    set_target_properties(scylladb_static PROPERTIES
      INTERFACE_LINK_LIBRARIES "${_STATIC_INTERFACE_LIBS}")
  endif()

  if(scylladb_static_requires_private)
    list(REMOVE_DUPLICATES scylladb_static_requires_private)
    string(JOIN " " scylladb_static_requires_private
      ${scylladb_static_requires_private})
  endif()

  if(scylladb_static_libs_private)
    list(REMOVE_DUPLICATES scylladb_static_libs_private)
    string(JOIN " " scylladb_static_libs_private
      ${scylladb_static_libs_private})
  endif()
endif()

#-------------------------------------
# Build metadata
#-------------------------------------

# Generate BUILD_INFO capturing the build environment. The script is
# bash-specific; if bash is unavailable the target is skipped silently.
find_program(BASH_EXECUTABLE bash)
if(BASH_EXECUTABLE)
  add_custom_command(
    OUTPUT ${CMAKE_BINARY_DIR}/BUILD_INFO
    COMMAND ${BASH_EXECUTABLE} ${CASS_ROOT_DIR}/ci/generate-build-info.sh
            ${CMAKE_BINARY_DIR}/BUILD_INFO
    WORKING_DIRECTORY ${CASS_ROOT_DIR}
    COMMENT "Generating BUILD_INFO"
    VERBATIM
  )
  add_custom_target(build_info ALL DEPENDS ${CMAKE_BINARY_DIR}/BUILD_INFO)
  if(TARGET scylladb_staticlib_target)
    add_dependencies(build_info scylladb_staticlib_target)
  elseif(TARGET scylladb_cdylib_target)
    add_dependencies(build_info scylladb_cdylib_target)
  endif()
endif()

#-------------------------------------
# Installation
#-------------------------------------

# Determine if the library directory needs to be determined
if(NOT DEFINED CMAKE_INSTALL_LIBDIR)
  if ("${CMAKE_SYSTEM_NAME}" MATCHES "Linux" AND ("${CMAKE_INSTALL_PREFIX}" STREQUAL "/usr" OR
    "${CMAKE_INSTALL_PREFIX}" STREQUAL "/usr/local"))
    if(EXISTS "/etc/debian_version")
      set (CMAKE_INSTALL_LIBDIR "lib/${CMAKE_LIBRARY_ARCHITECTURE}")
    elseif(EXISTS "/etc/redhat-release" OR EXISTS "/etc/fedora-release" OR
        EXISTS "/etc/slackware-version" OR EXISTS "/etc/gentoo-release")
      if(CMAKE_SIZEOF_VOID_P EQUAL 8)
        set (CMAKE_INSTALL_LIBDIR "lib64")
      else()
        set (CMAKE_INSTALL_LIBDIR "lib")
      endif()
    else()
      set (CMAKE_INSTALL_LIBDIR "lib")
    endif()
  else()
    set (CMAKE_INSTALL_LIBDIR "lib")
  endif()
endif()

# Create a binary directory executable and DLLs (windows only)
set(INSTALL_DLL_EXE_DIR "bin") # Determine the header install dir
if (CASS_INSTALL_HEADER_IN_SUBDIR)
  if (CASS_INSTALL_HEADER_SUBDIR_NAME)
    # User-specified include sub-dir
    set(INSTALL_HEADER_DIR "include/${CASS_INSTALL_HEADER_SUBDIR_NAME}")
  else()
    # Default subdir location is 'include/cassandra'
    set(INSTALL_HEADER_DIR "include/cassandra")
  endif()
else()
  # Default header install location is 'include'
  set(INSTALL_HEADER_DIR "include")
endif()

if(CASS_INSTALL_PKG_CONFIG)
  find_package(PkgConfig)
  if(PKG_CONFIG_FOUND)
    set(prefix ${CMAKE_INSTALL_PREFIX})
    set(exec_prefix ${CMAKE_INSTALL_PREFIX})
    set(libdir ${CMAKE_INSTALL_PREFIX}/${CMAKE_INSTALL_LIBDIR})
    set(includedir ${CMAKE_INSTALL_PREFIX}/${INSTALL_HEADER_DIR})
    set(version ${PROJECT_VERSION_STRING})
  endif()
endif()

# Determine if the header should be installed
if(CASS_INSTALL_HEADER)
  file(GLOB CASS_API_HEADER_FILES ${CASS_INCLUDE_DIR}/*.h)
  # Headers go to dev package
  install(FILES ${CASS_API_HEADER_FILES}
    DESTINATION ${INSTALL_HEADER_DIR}
    COMPONENT ${SCYLLA_DRIVER_DEV_COMPONENT_NAME})
endif()

# Install the dynamic/shared library
if(CASS_BUILD_SHARED)
  # Versioned shared library goes to runtime package
  if(WIN32)
    install(FILES ${CMAKE_BINARY_DIR}/${INSTALL_NAME_SHARED}
      DESTINATION ${INSTALL_DLL_EXE_DIR}
      COMPONENT ${SCYLLA_DRIVER_RUNTIME_COMPONENT_NAME}
      PERMISSIONS OWNER_EXECUTE OWNER_WRITE OWNER_READ GROUP_EXECUTE GROUP_READ WORLD_EXECUTE WORLD_READ)
    if(INSTALL_NAME_SHARED_IMPLIB)
      install(FILES ${CMAKE_BINARY_DIR}/${INSTALL_NAME_SHARED_IMPLIB}
        DESTINATION ${CMAKE_INSTALL_LIBDIR}
        COMPONENT ${SCYLLA_DRIVER_DEV_COMPONENT_NAME})
    endif()
  else()
    install(FILES $<TARGET_FILE:scylladb>
      DESTINATION ${CMAKE_INSTALL_LIBDIR}
      COMPONENT ${SCYLLA_DRIVER_RUNTIME_COMPONENT_NAME}
      PERMISSIONS OWNER_EXECUTE OWNER_WRITE OWNER_READ GROUP_EXECUTE GROUP_READ WORLD_EXECUTE WORLD_READ)
    # Major version symlink goes to runtime package
    install(FILES ${CMAKE_BINARY_DIR}/${INSTALL_NAME_SHARED_SYMLINK_VERSION}
      DESTINATION ${CMAKE_INSTALL_LIBDIR}
      COMPONENT ${SCYLLA_DRIVER_RUNTIME_COMPONENT_NAME})
    # Unversioned development symlink goes to dev package
    install(FILES ${CMAKE_BINARY_DIR}/${INSTALL_NAME_SHARED_SYMLINK_NO_VERSION}
      DESTINATION ${CMAKE_INSTALL_LIBDIR}
      COMPONENT ${SCYLLA_DRIVER_DEV_COMPONENT_NAME})
  endif()
  # pkg-config file goes to dev package
  if(CASS_INSTALL_PKG_CONFIG)
    if(PKG_CONFIG_FOUND)
      configure_file("${CMAKE_CURRENT_SOURCE_DIR}/scylladb.pc.in" "scylladb.pc" @ONLY)
      install(FILES "${CMAKE_CURRENT_BINARY_DIR}/scylladb.pc"
        DESTINATION "${CMAKE_INSTALL_LIBDIR}/pkgconfig"
        COMPONENT ${SCYLLA_DRIVER_DEV_COMPONENT_NAME})
    endif()
  endif()
endif()

# Static library goes to dev package
if(CASS_BUILD_STATIC)
  install(FILES $<TARGET_FILE:scylladb_static>
    DESTINATION ${CMAKE_INSTALL_LIBDIR}
    COMPONENT ${SCYLLA_DRIVER_DEV_COMPONENT_NAME})
  # Static library pkg-config file goes to dev package
  if(CASS_INSTALL_PKG_CONFIG)
    if(PKG_CONFIG_FOUND)
      configure_file("${CMAKE_CURRENT_SOURCE_DIR}/scylladb_static.pc.in" "scylladb_static.pc" @ONLY)
      install(FILES "${CMAKE_CURRENT_BINARY_DIR}/scylladb_static.pc"
        DESTINATION "${CMAKE_INSTALL_LIBDIR}/pkgconfig"
        COMPONENT ${SCYLLA_DRIVER_DEV_COMPONENT_NAME})
    endif()
  endif()
endif()

# Cassandra compatibility symlinks (for migration from Apache CPP Driver)
if(CASS_INSTALL_CASSANDRA_COMPAT AND NOT WIN32)
  if(CASS_BUILD_SHARED)
    install(FILES ${CMAKE_BINARY_DIR}/${CASSANDRA_COMPAT_SHARED_SYMLINK}
      DESTINATION ${CMAKE_INSTALL_LIBDIR}
      COMPONENT ${SCYLLA_DRIVER_DEV_COMPONENT_NAME})
  endif()
  if(CASS_BUILD_STATIC)
    install(FILES ${CMAKE_BINARY_DIR}/${CASSANDRA_COMPAT_STATIC_SYMLINK}
      DESTINATION ${CMAKE_INSTALL_LIBDIR}
      COMPONENT ${SCYLLA_DRIVER_DEV_COMPONENT_NAME})
  endif()
  if(CASS_INSTALL_PKG_CONFIG AND PKG_CONFIG_FOUND)
    configure_file("${CMAKE_CURRENT_SOURCE_DIR}/cassandra.pc.in" "cassandra.pc" @ONLY)
    install(FILES "${CMAKE_CURRENT_BINARY_DIR}/cassandra.pc"
      DESTINATION "${CMAKE_INSTALL_LIBDIR}/pkgconfig"
      COMPONENT ${SCYLLA_DRIVER_DEV_COMPONENT_NAME})
  endif()
endif()

#-------------------
# Uninstall target
#-------------------

configure_file(
  "${CASS_ROOT_DIR}/cmake_uninstall.cmake.in"
  "${CMAKE_CURRENT_BINARY_DIR}/cmake_uninstall.cmake"
  IMMEDIATE @ONLY)

add_custom_target(uninstall
  COMMAND ${CMAKE_COMMAND} -P ${CMAKE_CURRENT_BINARY_DIR}/cmake_uninstall.cmake)
