//! Secrets: seat tokens (random, shown once, kept only as SHA-256 digests) and server-chosen seeds.

use std::fmt;

use sha2::{Digest, Sha256};
use thiserror::Error;

/// Bytes of randomness in a token (256 bits).
const TOKEN_BYTES: usize = 32;
/// Characters in a token: two hex digits per byte.
pub const TOKEN_CHARS: usize = TOKEN_BYTES * 2;

/// The operating system's random source failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("the operating system's random source failed")]
pub struct RandomError;

/// A seat's credential: 256 random bits as lowercase hex. It never appears in `Debug` output or logs.
#[derive(Clone, PartialEq, Eq)]
pub struct SeatToken(String);

impl SeatToken {
    /// A fresh token from the operating system's CSPRNG.
    pub fn generate() -> Result<SeatToken, RandomError> {
        let mut bytes = [0u8; TOKEN_BYTES];
        getrandom::fill(&mut bytes).map_err(|_| RandomError)?;
        let hex = bytes
            .iter()
            .flat_map(|byte| [byte >> 4, byte & 0x0f])
            .map(|nibble| char::from_digit(u32::from(nibble), 16).unwrap_or('0'))
            .collect();
        Ok(SeatToken(hex))
    }

    /// The token text, for the one response that hands it to its owner.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// The digest the registry stores in place of the token.
    pub fn digest(&self) -> TokenDigest {
        TokenDigest::of(&self.0)
    }
}

impl fmt::Debug for SeatToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SeatToken(<redacted>)")
    }
}

/// The SHA-256 digest of a token; comparing digests leaks nothing about the token.
#[derive(Clone, Copy)]
pub struct TokenDigest([u8; 32]);

impl TokenDigest {
    /// The digest of `presented`, as sent by a client.
    pub fn of(presented: &str) -> TokenDigest {
        TokenDigest(Sha256::digest(presented.as_bytes()).into())
    }

    /// A digest read back from storage.
    pub fn from_bytes(bytes: [u8; 32]) -> TokenDigest {
        TokenDigest(bytes)
    }

    /// The raw digest, for storage.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Whether both digests are equal, without stopping at the first difference.
    pub fn matches(&self, other: &TokenDigest) -> bool {
        self.0
            .iter()
            .zip(&other.0)
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0
    }
}

impl fmt::Debug for TokenDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TokenDigest(..)")
    }
}

/// Whether `text` has the shape of a token, so lookups can skip hashing obvious junk.
pub fn looks_like_token(text: &str) -> bool {
    text.len() == TOKEN_CHARS && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// A game seed drawn from the operating system's CSPRNG.
pub fn random_seed() -> Result<u64, RandomError> {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).map_err(|_| RandomError)?;
    Ok(u64::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    //! Goal: tokens are long, random, well-formed and never printed, and their digest is the SHA-256 of the token text.
    use std::collections::HashSet;

    use super::*;
    use rstest::rstest;

    fn hex(digest: &TokenDigest) -> String {
        use std::fmt::Write;
        digest.0.iter().fold(String::new(), |mut out, byte| {
            write!(out, "{byte:02x}").unwrap();
            out
        })
    }

    #[test]
    fn tokens_are_distinct_and_well_formed() {
        let tokens: Vec<SeatToken> = (0..500).map(|_| SeatToken::generate().unwrap()).collect();
        assert!(tokens.iter().all(|t| looks_like_token(t.expose())));
        let distinct: HashSet<&str> = tokens.iter().map(SeatToken::expose).collect();
        assert_eq!(distinct.len(), tokens.len());
        const { assert!(TOKEN_BYTES * 8 >= 128) };
    }

    #[test]
    fn debug_output_hides_the_token() {
        let token = SeatToken::generate().unwrap();
        let shown = format!("{token:?} {:?}", token.digest());
        assert!(!shown.contains(token.expose()));
    }

    #[test]
    fn digest_is_sha256_of_the_text() {
        assert_eq!(
            hex(&TokenDigest::of("abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn digests_match_only_their_own_token() {
        let a = SeatToken::generate().unwrap();
        let b = SeatToken::generate().unwrap();
        assert!(a.digest().matches(&TokenDigest::of(a.expose())));
        assert!(!a.digest().matches(&b.digest()));
    }

    #[test]
    fn digests_survive_storage() {
        let token = SeatToken::generate().unwrap();
        let stored = *token.digest().as_bytes();
        assert!(TokenDigest::from_bytes(stored).matches(&TokenDigest::of(token.expose())));
        assert!(!TokenDigest::from_bytes([0; 32]).matches(&token.digest()));
    }

    #[rstest]
    #[case("", false)]
    #[case("abc", false)]
    #[case(&"a".repeat(63), false)]
    #[case(&"a".repeat(65), false)]
    #[case(&"A".repeat(64), false)]
    #[case(&"g".repeat(64), false)]
    #[case(&format!("{}é", "a".repeat(62)), false)]
    #[case(&"0123456789abcdef".repeat(4), true)]
    fn token_shapes(#[case] text: &str, #[case] expected: bool) {
        assert_eq!(looks_like_token(text), expected);
    }

    #[test]
    fn seeds_vary() {
        let seeds: HashSet<u64> = (0..50).map(|_| random_seed().unwrap()).collect();
        assert!(seeds.len() > 40);
    }
}
