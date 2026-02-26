cmake_minimum_required(VERSION 3.18)
project(tunhelper C)

add_library(tunhelper SHARED tun_helper.c)
target_link_libraries(tunhelper log)
