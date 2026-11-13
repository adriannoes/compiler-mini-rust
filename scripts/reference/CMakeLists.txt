cmake_minimum_required(VERSION 3.20)
project(minic_comparison LANGUAGES CXX)

enable_testing()
add_subdirectory("${REFERENCE_SOURCE}" reference)

add_executable(compare_limits limits.cpp)
target_compile_features(compare_limits PRIVATE cxx_std_17)
target_link_libraries(compare_limits PRIVATE minic_optimizer minic_vm)
