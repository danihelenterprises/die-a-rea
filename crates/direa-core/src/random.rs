//! Deterministic randomization.
//!
//! Every randomized transformation is driven by a seeded SplitMix64 generator.
//! When a manifest supplies a non-zero seed (or a seed is passed on the command
//! line) builds are reproducible; otherwise a seed is derived from stable
//! project metadata so that the same source produces the same "random" layout.
//! Games of chance that are reproducible are easier to debug and to verify.

/// A small, dependency-free SplitMix64 PRNG.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng {
            state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15),
        }
    }

    /// Seed from stable string parts (e.g. project name, version, target).
    pub fn from_parts(parts: &[&str]) -> Self {
        Rng::new(derive_seed(parts))
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform-ish in `[0, n)`. Returns 0 when `n == 0`.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next_u64() % n
        }
    }

    /// Uniform-ish integer in `[min, max)`.
    pub fn range(&mut self, min: u64, max: u64) -> u64 {
        if max <= min {
            min
        } else {
            min + self.below(max - min)
        }
    }

    /// True with probability `p` (clamped to `[0, 1]`).
    pub fn bool(&mut self, p: f64) -> bool {
        let p = p.clamp(0.0, 1.0);
        (self.next_u64() as f64 / u64::MAX as f64) < p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }

    /// Lowercase hex string of `len` characters.
    pub fn hex(&mut self, len: usize) -> String {
        let alphabet = b"0123456789abcdef";
        (0..len)
            .map(|_| alphabet[self.below(16) as usize] as char)
            .collect()
    }

    /// A plausible JS-style identifier (never a reserved word).
    pub fn ident(&mut self, len: usize) -> String {
        let mut s = String::with_capacity(len + 1);
        let first = b"abcdefghijklmnopqrstuvwxyz_";
        s.push(first[self.below(first.len() as u64) as usize] as char);
        let rest = b"abcdefghijklmnopqrstuvwxyz0123456789_";
        for _ in 1..len.max(1) {
            s.push(rest[self.below(rest.len() as u64) as usize] as char);
        }
        s
    }

    /// A plausible decoy identifier built from the word list, e.g.
    /// `validate_license_x7f2`.
    pub fn word_ident(&mut self) -> String {
        let a = self.pick(WORDLIST);
        let b = self.pick(WORDLIST);
        let suffix = self.hex(4);
        format!("{a}_{b}_{suffix}")
    }

    /// A plausible-looking secret/URL/token decoy string.
    pub fn decoy_string(&mut self) -> String {
        match self.below(5) {
            0 => format!("AKIA{}", self.hex(16).to_uppercase()),
            1 => format!(
                "https://{}.example.com/v{}/{}",
                self.pick(WORDLIST),
                self.range(1, 9),
                self.hex(12)
            ),
            2 => format!("sk_live_{}", self.hex(24)),
            3 => format!(
                "{}.{}.{}.{}",
                self.range(1, 255),
                self.range(0, 255),
                self.range(0, 255),
                self.range(1, 255)
            ),
            _ => format!(
                "{}-{}-{}",
                self.pick(WORDLIST),
                self.hex(8),
                self.pick(WORDLIST)
            ),
        }
    }

    /// A readable phrase built from the word list, for padding/decoy comments.
    pub fn phrase(&mut self, words: usize) -> String {
        (0..words)
            .map(|_| *self.pick(WORDLIST))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// FNV-1a over the given parts, used to derive a stable default seed.
pub fn derive_seed(parts: &[&str]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for b in part.as_bytes() {
            hash ^= *b as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// Plausible words used to synthesize decoy identifiers and strings.
pub const WORDLIST: &[&str] = &[
    "license", "serial", "token", "config", "secret", "key", "user", "admin", "decrypt", "verify",
    "session", "auth", "cache", "flush", "buffer", "handler", "dispatch", "route", "node", "graph",
    "packet", "frame", "cipher", "salt", "nonce", "digest", "entropy", "module", "plugin",
    "engine", "runtime", "kernel", "socket", "stream", "channel", "registry", "manifest", "policy",
    "quota", "tenant", "billing", "invoice", "checkout", "wallet", "ledger", "sync", "merge",
    "delta", "index",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_same_seed() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn ids_and_tokens_are_well_formed() {
        let mut r = Rng::new(7);
        let id = r.ident(8);
        assert_eq!(id.len(), 8);
        assert!(id.chars().next().unwrap().is_ascii_alphabetic() || id.starts_with('_'));
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));

        let word = r.word_ident();
        assert!(word.contains('_'));
    }

    #[test]
    fn derived_seed_is_stable() {
        assert_eq!(derive_seed(&["a", "b"]), derive_seed(&["a", "b"]));
        assert_ne!(derive_seed(&["a", "b"]), derive_seed(&["a", "c"]));
    }
}
