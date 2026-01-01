cmake_minimum_required(VERSION 3.16)

if(NOT DEFINED CPACK_BUILD_DIR OR CPACK_BUILD_DIR STREQUAL "")
  message(FATAL_ERROR "CPACK_BUILD_DIR must point to the CPack build directory")
endif()

if(NOT DEFINED CPACK_BUILD_CONFIG OR CPACK_BUILD_CONFIG STREQUAL "")
  set(CPACK_BUILD_CONFIG "Release")
endif()

set(_cpack_config "${CPACK_BUILD_DIR}/CPackConfig.cmake")
if(NOT EXISTS "${_cpack_config}")
  message(FATAL_ERROR "Could not find CPack configuration file at ${_cpack_config}")
endif()

include("${_cpack_config}")

if(NOT DEFINED CPACK_COMPONENTS_ALL OR CPACK_COMPONENTS_ALL STREQUAL "")
  message(FATAL_ERROR "No components configured for packaging")
endif()

if(NOT DEFINED CPACK_PACKAGE_FILE_NAME OR CPACK_PACKAGE_FILE_NAME STREQUAL "")
  message(FATAL_ERROR "CPACK_PACKAGE_FILE_NAME is empty")
endif()

set(_cpack_executable "cpack")
if(DEFINED CPACK_EXECUTABLE AND NOT CPACK_EXECUTABLE STREQUAL "")
  set(_cpack_executable "${CPACK_EXECUTABLE}")
endif()

string(REGEX MATCH "^(.+)_([0-9].*)$" _matched "${CPACK_PACKAGE_FILE_NAME}")
if(NOT _matched)
  message(FATAL_ERROR "Unexpected CPACK_PACKAGE_FILE_NAME format: ${CPACK_PACKAGE_FILE_NAME}")
endif()
set(_base_name "${CMAKE_MATCH_1}")
set(_suffix "${CMAKE_MATCH_2}")

foreach(_component IN LISTS CPACK_COMPONENTS_ALL)
  set(_package_name "${_base_name}_${_suffix}")
  if(NOT _component STREQUAL "${_base_name}")
    string(REPLACE "${_base_name}" "${_component}" _package_name "${_package_name}")
    if(_package_name STREQUAL "${_base_name}_${_suffix}")
      set(_package_name "${_component}_${_suffix}")
    endif()
  endif()

  message(STATUS "Generating productbuild package for component '${_component}' as '${_package_name}.pkg'")

  execute_process(
    COMMAND "${_cpack_executable}" -G productbuild -C "${CPACK_BUILD_CONFIG}"
            -D "CPACK_COMPONENTS_ALL=${_component}"
            -D "CPACK_PACKAGE_FILE_NAME=${_package_name}"
            --config "${_cpack_config}"
    WORKING_DIRECTORY "${CPACK_BUILD_DIR}"
    RESULT_VARIABLE _result)
  if(NOT _result EQUAL 0)
    message(FATAL_ERROR "cpack failed for component ${_component} with exit code ${_result}")
  endif()
endforeach()
