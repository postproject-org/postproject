# Creates a fresh work directory for one guide-example run.
#
#   cmake -DWORK_DIR=... -DMEDIA_FIXTURE=... -P prepare-workdir.cmake
#
# rushes/A001.mov is imported and later moved into moved/ to exercise
# relocation; renders/shot010 is an image sequence with frame 1003 missing.
foreach(variable IN ITEMS WORK_DIR MEDIA_FIXTURE)
    if(NOT DEFINED ${variable})
        message(FATAL_ERROR "${variable} is required")
    endif()
endforeach()

file(REMOVE_RECURSE "${WORK_DIR}")
file(MAKE_DIRECTORY "${WORK_DIR}/rushes" "${WORK_DIR}/moved" "${WORK_DIR}/renders/shot010")
file(COPY_FILE "${MEDIA_FIXTURE}" "${WORK_DIR}/rushes/A001.mov")
file(WRITE "${WORK_DIR}/scene.ppscene" "rushes/A001.mov\n")
foreach(frame IN ITEMS 1001 1002 1004)
    file(WRITE "${WORK_DIR}/renders/shot010/shot010.${frame}.exr" "frame ${frame}\n")
endforeach()
