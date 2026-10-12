//! Byte-level scanning helpers shared by the verify probes and the build
//! pipeline, so both agree on what counts as a recoverable "string".
//!
//! The unit is an **identifier/literal token**: a maximal run of
//! `[A-Za-z0-9_$]` at least `min` bytes long. Unlike contiguous printable runs,
//! tokens do not collapse a minified, single-line file into one blob (lesson
//! K-013), and they match how symbol and literal names actually appear in
//! binaries and bundles.
//!
//! The `strings` axis consumes a *set* of these tokens so it can measure how
//! much of the original signal survives verbatim (see `token_runs` and
//! ADR-0011). That approach handles string encoding without heuristics: an
//! encoded blob is simply not one of the original tokens.

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

/// Collect every distinct token of at least `min` bytes.
///
/// Used to measure verbatim survival: the fraction of the baseline's tokens that
/// still appear in the hardened artifact. Ordering is not meaningful.
pub fn token_runs(bytes: &[u8], min: usize) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut len = 0usize;
    for (i, &b) in bytes.iter().enumerate() {
        if is_token_byte(b) {
            if len == 0 {
                start = i;
            }
            len += 1;
        } else {
            if len >= min {
                out.push(&bytes[start..start + len]);
            }
            len = 0;
        }
    }
    if len >= min {
        out.push(&bytes[start..start + len]);
    }
    out
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
    fn separators_break_tokens() {
        // Punctuation is not a token byte.
        assert_eq!(count_tokens(b"abcdef.ghijkl", DEFAULT_MIN_TOKEN), 2);
    }

    #[test]
    fn token_runs_returns_slices() {
        let src = b"\x00deadbeefdeadbeef\x00SUPER_SECRET_LICENSE\x00ok\x00";
        let runs = token_runs(src, DEFAULT_MIN_TOKEN);
        assert_eq!(runs.len(), 2);
        assert!(runs.contains(&&b"deadbeefdeadbeef"[..]));
        assert!(runs.contains(&&b"SUPER_SECRET_LICENSE"[..]));
    }

    #[test]
    fn token_runs_agrees_with_count() {
        let src = b"aaa bbbb ccccc dddddd eeeeeee";
        assert_eq!(
            token_runs(src, DEFAULT_MIN_TOKEN).len() as u64,
            count_tokens(src, DEFAULT_MIN_TOKEN)
        );
    }
}
