# Keep native harnesses tied to the Cargo implementation they exercise.
if(DEFINED FERRIC_ARCHIVE_TO_CHECK)
    if(NOT EXISTS "${FERRIC_ARCHIVE_TO_CHECK}")
        message(FATAL_ERROR "Ferric engine archive is missing. Build ferric-browser-engine-qt explicitly with cargo build -p ferric-browser-engine-qt --lib (and the selected profile).")
    endif()
    set(inputs "${FERRIC_SOURCE_ROOT}/Cargo.toml" "${FERRIC_SOURCE_ROOT}/Cargo.lock")
    foreach(crate IN ITEMS ferric-browser-engine-qt ferric-browser-application
            ferric-browser-runtime ferric-browser-core ferric-browser-config
            ferric-browser-storage ferric-browser-ipc)
        set(base "${FERRIC_SOURCE_ROOT}/crates/${crate}")
        list(APPEND inputs "${base}/Cargo.toml" "${base}/build.rs")
        file(GLOB_RECURSE sources LIST_DIRECTORIES FALSE
            "${base}/src/*.rs" "${base}/src/*.cpp" "${base}/src/*.h" "${base}/src/*.hpp" "${base}/src/*.lua")
        list(APPEND inputs ${sources})
    endforeach()
    file(GLOB_RECURSE resources LIST_DIRECTORIES FALSE
        "${FERRIC_SOURCE_ROOT}/crates/ferric-browser-engine-qt/qml/*.qml"
        "${FERRIC_SOURCE_ROOT}/crates/ferric-browser-engine-qt/qml/*.js")
    list(APPEND inputs ${resources})
    foreach(input IN LISTS inputs)
        if(EXISTS "${input}" AND "${input}" IS_NEWER_THAN "${FERRIC_ARCHIVE_TO_CHECK}")
            file(RELATIVE_PATH relative "${FERRIC_SOURCE_ROOT}" "${input}")
            message(FATAL_ERROR "Ferric engine archive predates ${relative}. Rebuild it explicitly with cargo build -p ferric-browser-engine-qt --lib (and the selected profile), then relink this harness. Building the browser alone may leave the top-level archive stale.")
        endif()
    endforeach()
    if(DEFINED FERRIC_ARCHIVE_STAMP)
        file(SHA256 "${FERRIC_ARCHIVE_TO_CHECK}" digest)
        set(previous "")
        if(EXISTS "${FERRIC_ARCHIVE_STAMP}")
            file(READ "${FERRIC_ARCHIVE_STAMP}" previous)
        endif()
        if(NOT previous STREQUAL digest)
            file(WRITE "${FERRIC_ARCHIVE_STAMP}" "${digest}")
        endif()
    endif()
    return()
endif()

function(ferric_attach_archive_check directory archive)
    get_property(targets DIRECTORY "${directory}" PROPERTY BUILDSYSTEM_TARGETS)
    foreach(target IN LISTS targets)
        get_target_property(libraries "${target}" LINK_LIBRARIES)
        if("${archive}" IN_LIST libraries)
            add_dependencies("${target}" ferric_engine_archive_current)
            set_property(TARGET "${target}" APPEND PROPERTY LINK_DEPENDS "${CMAKE_BINARY_DIR}/ferric-engine-archive.sha256")
        endif()
    endforeach()
    get_property(children DIRECTORY "${directory}" PROPERTY SUBDIRECTORIES)
    foreach(child IN LISTS children)
        ferric_attach_archive_check("${child}" "${archive}")
    endforeach()
endfunction()

function(ferric_require_current_engine_archive archive)
    if(NOT archive)
        return()
    endif()
    get_filename_component(root "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/../../.." ABSOLUTE)
    if(NOT TARGET ferric_engine_archive_current)
        set(stamp "${CMAKE_BINARY_DIR}/ferric-engine-archive.sha256")
        if(NOT EXISTS "${stamp}")
            file(WRITE "${stamp}" "")
        endif()
        add_custom_target(ferric_engine_archive_current
            COMMAND "${CMAKE_COMMAND}"
                "-DFERRIC_ARCHIVE_TO_CHECK=${archive}" "-DFERRIC_SOURCE_ROOT=${root}"
                "-DFERRIC_ARCHIVE_STAMP=${stamp}"
                -P "${CMAKE_CURRENT_FUNCTION_LIST_FILE}"
            BYPRODUCTS "${stamp}"
            COMMENT "Checking Cargo engine archive against workspace implementation inputs"
            VERBATIM)
    endif()
    ferric_attach_archive_check("${CMAKE_CURRENT_SOURCE_DIR}" "${archive}")
endfunction()
