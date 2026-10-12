//! Byte-level scanning helpers shared by the verify probes and the build
//! pipeline, so both agree on what counts as a recoverable "string".
//!
//! The unit is an **identifier/literal token**: a maximal run of
//! `[A-Za-z0-9_$]` at least `min` bytes long. Unlike contiguous printable runs,
//! tokens do not collapse a minified, single-line file into one blob (lesson
//! K-013), and they match how symbol and literal names actually appear in
//! binaries and bundles.

/// Default minimum token length.
pub const DEFAULT_MIN_TOKEN: usize = 6;

/// Bytes that may appear inside a token.
#[inline]
pub fn is_token_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Count maximal token runs of at least `min` bytes.
pub fn count_tokens(bytes: &[u8], min: usize) -> u64 {
    let mut count = 0u64;
    let mut len = 0usize;
    for &b in bytes {
        if is_token_byte(b) {
            len += 1;
        } else {
            if len >= min {
                count += 1;
            }
            len = 0;
        }
    }
    if len >= min {
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_tokens_not_lines() {
        // One minified line, several long tokens, should not collapse to one.
        let src = b"var abcdef=123456;function ghijkl(){return mnoPQR;}";
        assert!(count_tokens(src, DEFAULT_MIN_TOKEN) >= 3);
    }

    #[test]
    fn short_tokens_are_ignored() {
        assert_eq!(count_tokens(b"a=b+c", DEFAULT_MIN_TOKEN), 0);
    }

    #[test]
    fn skips_short_and_keeps_long() {
        let src = b"ok longenough xx  anotherlong";
        assert_eq!(count_tokens(src, DEFAULT_MIN_TOKEN), 2);
    }

    #[test]
    fn separators_break_tokens() {
        // Punctuation is not a token byte.
        assert_eq!(count_tokens(b"abcdef.ghijkl", DEFAULT_MIN_TOKEN), 2);
    }
}
