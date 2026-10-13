//! C ABI for die-a-rea (optional).
//!
//! Lets other languages call a small, stable surface: the version string, the
//! runtime guard check, and in-place native metadata stripping. See
//! `direa_ffi.h`. Not every feature is exposed here; the CLI is the full
//! interface.

use std::os::raw::{c_char, c_int};

/// Library version, as a NUL-terminated C string (do not free).
#[no_mangle]
pub extern "C" fn direa_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

/// Returns 1 if runtime instrumentation is detected in this process, else 0.
#[no_mangle]
pub extern "C" fn direa_guard_detect() -> c_int {
    if direa_core::guard::detect().instrumented {
        1
    } else {
        0
    }
}

/// Strip symbol/debug metadata from a native artifact in place.
///
/// Returns 1 if the buffer changed, 0 if unchanged, -1 on invalid input.
///
/// # Safety
/// `ptr` must point to `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn direa_strip_native(ptr: *mut u8, len: usize) -> c_int {
    if ptr.is_null() || len == 0 {
        return -1;
    }
    let buffer = std::slice::from_raw_parts_mut(ptr, len);
    let report = direa_core::native_post::strip_symbols(buffer);
    if report.changed {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_nonempty() {
        let p = direa_version();
        assert!(!p.is_null());
    }

    #[test]
    fn guard_detect_returns_bool() {
        let v = direa_guard_detect();
        assert!(v == 0 || v == 1);
    }

    #[test]
    fn strip_handles_input() {
        assert_eq!(unsafe { direa_strip_native(std::ptr::null_mut(), 0) }, -1);
        let mut elf = b"\x7fELF\x02\x01\x01\x00rest".to_vec();
        let r = unsafe { direa_strip_native(elf.as_mut_ptr(), elf.len()) };
        assert!(r == 0 || r == 1);
    }
}
