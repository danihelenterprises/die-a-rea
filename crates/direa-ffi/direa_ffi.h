/* direa_ffi.h — C ABI for die-a-rea (link against libdirea_ffi). */
#ifndef DIREA_FFI_H
#define DIREA_FFI_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Library version as a NUL-terminated string (do not free). */
const char *direa_version(void);

/* 1 if runtime instrumentation is detected in this process, else 0. */
int direa_guard_detect(void);

/* Strip symbol/debug metadata from a native artifact in place.
 * Returns 1 if the buffer changed, 0 if unchanged, -1 on invalid input. */
int direa_strip_native(unsigned char *ptr, size_t len);

#ifdef __cplusplus
}
#endif

#endif /* DIREA_FFI_H */
