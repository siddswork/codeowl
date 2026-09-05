//! Stable content hashing for `source_hash`/`interface_hash`.
//!
//! Deliberately not `std::hash::Hasher` (`DefaultHasher`'s output is only
//! documented to be stable within one build of one Rust version — not
//! something safe to persist to `.codeowl/graph` and compare against on a
//! later run, possibly after a toolchain upgrade). `blake3` is fast, has a
//! fixed algorithm, and produces the same digest on any machine.

/// Hash arbitrary text into a stable hex string.
pub fn hash_text(input: &str) -> String {
    blake3::hash(input.as_bytes()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_input_hashes_the_same() {
        assert_eq!(hash_text("hello"), hash_text("hello"));
    }

    #[test]
    fn different_input_hashes_differently() {
        assert_ne!(hash_text("hello"), hash_text("world"));
    }
}
