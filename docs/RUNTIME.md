# Runtime guard

`die-a-rea` can detect runtime instrumentation (debugger, Frida, injected
libraries) in two places: the **analysis environment** (`direa_core::guard`,
scored by `verify --with-runtime`) and **inside your artifacts**.

## JavaScript / Electron

`build` appends a behavior-safe guard to JS/Electron files when
`policy.anti_instrumentation` is set (High/Paranoid profiles). It never throws or
blocks; it records:

```js
globalThis.__DIREA_GUARD__ = { instrumented: boolean, findings: [...] };
```

and calls `globalThis.__DIREA_ON_INSTRUMENTED__(findings)` if you define it.
See `guard::JS_GUARD_SOURCE` (ADR-0014, K-028).

## Native programs

There is no safe post-build injection into a native binary, so the native guard
is a small **linkable C library** (`runtime/guard/`, MIT). Compile and link it
into your program:

```c
#include "direa_guard.h"

int main(void) {
    struct direa_guard_result r;
    if (direa_guard_check(&r)) {
        /* instrumented: r.findings lists the signals */
    }
    return 0;
}
```

```bash
cc -I runtime/guard your_app.c runtime/guard/direa_guard.c -o your_app
```

`direa_guard_check` returns 1 when instrumentation is detected. Detection is
platform-specific: `TracerPid`/process maps on Linux, `IsDebuggerPresent` on
Windows, `P_TRACED` on macOS, plus injected-library environment variables
(`LD_PRELOAD`, `DYLD_INSERT_LIBRARIES`, `FRIDA`, `NODE_OPTIONS=--inspect`) on all
platforms. The
library ships a weak no-op `direa_guard_on_instrumented`; define your own to
react (report, degrade, exit — your choice; never destructive by default).

## Limits

- Detection is heuristic and evadable; it raises cost, not certainty.
- The native guard is link-time, so it only protects programs you build with it.
- It does not attach to or modify any other process (ADR-0001).
