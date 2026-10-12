/*
 * direa_guard.h — embeddable runtime instrumentation guard for native programs.
 *
 * Link this into your program to detect debuggers / Frida / injected libraries
 * at runtime, mirroring `direa_core::guard`. It never throws or blocks; it fills
 * a result struct and calls an optional hook you define.
 *
 *   #include "direa_guard.h"
 *   struct direa_guard_result r;
 *   if (direa_guard_check(&r)) { ... }
 *
 * Provide your own `direa_guard_on_instrumented` to react (the default is a
 * weak no-op). MIT licensed, like the rest of die-a-rea (ADR-0018).
 */
#ifndef DIREA_GUARD_H
#define DIREA_GUARD_H

#ifdef __cplusplus
extern "C" {
#endif

#define DIREA_GUARD_MAX_FINDINGS 8
#define DIREA_GUARD_FINDING_LEN 64

struct direa_guard_result {
    int instrumented;
    int finding_count;
    char findings[DIREA_GUARD_MAX_FINDINGS][DIREA_GUARD_FINDING_LEN];
};

/* Returns 1 when instrumentation is detected, 0 otherwise. Never throws. */
int direa_guard_check(struct direa_guard_result *out);

/* Optional notification hook. The library ships a weak no-op default; define
 * your own to react to detection. */
void direa_guard_on_instrumented(const struct direa_guard_result *result);

#ifdef __cplusplus
}
#endif

#endif /* DIREA_GUARD_H */
