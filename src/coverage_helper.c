// Coverage helper for macOS and Linux
// Provides access to LLVM coverage sections

#include <stdint.h>
#include <stddef.h>

// Weak external references to LLVM coverage sections
// These are filled by the compiler when -C instrument-coverage is used

// Counter section
extern uint64_t* __llvm_profile_begin_counters(void) __attribute__((weak));
extern uint64_t* __llvm_profile_end_counters(void) __attribute__((weak));

// Data section (profile metadata) - these are FUNCTIONS in compiler-rt
extern const void* __llvm_profile_begin_data(void) __attribute__((weak));
extern const void* __llvm_profile_end_data(void) __attribute__((weak));

// Names section (compressed function names) - these are FUNCTIONS in compiler-rt
extern const char* __llvm_profile_begin_names(void) __attribute__((weak));
extern const char* __llvm_profile_end_names(void) __attribute__((weak));

// Public interface for counter access
uint64_t* coverage_begin_counters(void) {
    if (__llvm_profile_begin_counters == 0) {
        return 0;
    }
    return __llvm_profile_begin_counters();
}

uint64_t* coverage_end_counters(void) {
    if (__llvm_profile_end_counters == 0) {
        return 0;
    }
    return __llvm_profile_end_counters();
}

void coverage_reset_counters(void) {
    uint64_t* begin = coverage_begin_counters();
    uint64_t* end = coverage_end_counters();
    if (begin && end && begin < end) {
        uintptr_t begin_addr = (uintptr_t)begin;
        uintptr_t end_addr = (uintptr_t)end;
        if (end_addr > begin_addr) {
            size_t num_counters = (end_addr - begin_addr) / sizeof(uint64_t);
            if (num_counters > 0 && num_counters < 10000000) {
                for (size_t i = 0; i < num_counters && i < 100000; i++) {
                    begin[i] = 0;
                }
            }
        }
    }
}

size_t coverage_get_num_counters(void) {
    uint64_t* begin = coverage_begin_counters();
    uint64_t* end = coverage_end_counters();
    if (begin && end && begin < end) {
        return (size_t)(end - begin);
    }
    return 0;
}

// Profile data section access
// These are FUNCTIONS in compiler-rt, not variables
const char* coverage_begin_data(void) {
    if (__llvm_profile_begin_data == 0) {
        return 0;
    }
    return (const char*)__llvm_profile_begin_data();
}

const char* coverage_end_data(void) {
    if (__llvm_profile_end_data == 0) {
        return 0;
    }
    return (const char*)__llvm_profile_end_data();
}

size_t coverage_get_data_size(void) {
    const char* begin = coverage_begin_data();
    const char* end = coverage_end_data();
    if (begin && end && begin < end) {
        return (size_t)(end - begin);
    }
    return 0;
}

// Names section access
// These are FUNCTIONS in compiler-rt, not variables
const char* coverage_begin_names(void) {
    if (__llvm_profile_begin_names == 0) {
        return 0;
    }
    return __llvm_profile_begin_names();
}

const char* coverage_end_names(void) {
    if (__llvm_profile_end_names == 0) {
        return 0;
    }
    return __llvm_profile_end_names();
}

size_t coverage_get_names_size(void) {
    const char* begin = coverage_begin_names();
    const char* end = coverage_end_names();
    if (begin && end && begin < end) {
        return (size_t)(end - begin);
    }
    return 0;
}

// Check if coverage is available
int coverage_is_available(void) {
    return __llvm_profile_begin_counters != 0 && __llvm_profile_end_counters != 0;
}
