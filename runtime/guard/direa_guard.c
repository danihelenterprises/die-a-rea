/* direa_guard.c — see direa_guard.h. Portable C11; no dependencies. */
#include "direa_guard.h"

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int add_finding(struct direa_guard_result *r, const char *msg) {
    if (r->finding_count >= DIREA_GUARD_MAX_FINDINGS) {
        return 0;
    }
    strncpy(r->findings[r->finding_count], msg, DIREA_GUARD_FINDING_LEN - 1);
    r->findings[r->finding_count][DIREA_GUARD_FINDING_LEN - 1] = '\0';
    r->finding_count++;
    return 1;
}

static int contains_ci(const char *haystack, const char *needle) {
    size_t nlen;
    if (haystack == NULL || needle == NULL) {
        return 0;
    }
    nlen = strlen(needle);
    if (nlen == 0) {
        return 0;
    }
    for (const char *p = haystack; *p; ++p) {
        size_t i = 0;
        while (i < nlen && p[i] != '\0' &&
               tolower((unsigned char)p[i]) == tolower((unsigned char)needle[i])) {
            i++;
        }
        if (i == nlen) {
            return 1;
        }
    }
    return 0;
}

#if defined(__linux__)
static void scan_status(struct direa_guard_result *r) {
    FILE *f = fopen("/proc/self/status", "r");
    char line[256];
    if (f == NULL) {
        return;
    }
    while (fgets(line, sizeof line, f) != NULL) {
        if (strncmp(line, "TracerPid:", 10) == 0 && atoi(line + 10) != 0) {
            add_finding(r, "TracerPid");
        }
    }
    fclose(f);
}

static void scan_maps(struct direa_guard_result *r) {
    FILE *f = fopen("/proc/self/maps", "r");
    char line[512];
    if (f == NULL) {
        return;
    }
    while (fgets(line, sizeof line, f) != NULL) {
        if (contains_ci(line, "frida") || contains_ci(line, "gum-js") ||
            contains_ci(line, "gadget") || contains_ci(line, "lldb")) {
            add_finding(r, "map");
        }
    }
    fclose(f);
}
#endif

static void scan_env(struct direa_guard_result *r) {
    static const char *keys[] = {
        "LD_PRELOAD", "DYLD_INSERT_LIBRARIES", "FRIDA", "NODE_OPTIONS"};
    size_t i;
    for (i = 0; i < sizeof(keys) / sizeof(keys[0]); ++i) {
        const char *value = getenv(keys[i]);
        if (value == NULL) {
            continue;
        }
        if (strcmp(keys[i], "FRIDA") == 0) {
            add_finding(r, "env:FRIDA");
        } else if (strcmp(keys[i], "NODE_OPTIONS") == 0) {
            if (contains_ci(value, "--inspect")) {
                add_finding(r, "NODE_OPTIONS:--inspect");
            }
        } else if (contains_ci(value, "frida") || contains_ci(value, "gum") ||
                   contains_ci(value, "gadget")) {
            add_finding(r, "injected-lib");
        }
    }
}

#if defined(__GNUC__)
__attribute__((weak))
#endif
void direa_guard_on_instrumented(const struct direa_guard_result *result) {
    (void)result;
}

int direa_guard_check(struct direa_guard_result *out) {
    if (out == NULL) {
        return 0;
    }
    memset(out, 0, sizeof *out);
#if defined(__linux__)
    scan_status(out);
    scan_maps(out);
#endif
    scan_env(out);
    out->instrumented = out->finding_count > 0 ? 1 : 0;
    if (out->instrumented) {
        direa_guard_on_instrumented(out);
    }
    return out->instrumented;
}
