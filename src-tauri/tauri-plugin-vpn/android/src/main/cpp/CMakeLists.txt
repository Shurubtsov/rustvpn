cmake_minimum_required(VERSION 3.18)
project(hevjni C)

add_library(hevjni SHARED hev_jni.c)
target_link_libraries(hevjni log dl)
