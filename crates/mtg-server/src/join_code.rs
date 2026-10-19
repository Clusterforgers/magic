//! Join codes: eight characters a person can read out or type, shown once to the game's creator and
//! kept only as a SHA-256 digest, like a seat token.
//!
//! A code has 40 bits: 32 symbols, 5 bits each. The alphabet leaves out the characters that are
//! mistaken for each other (`I` and `1`, `O` and `0`). Guessing one is bounded by the throttle
//! (`Registry::find_by_code`), not by the size of the code.

use std::fmt;

use crate::secret::{RandomError, TokenDigest};

/// The 32 symbols of a code: capital letters and digits without `I`, `O`, `0` and `1`.
pub const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
/// Characters in a code.
pub const CODE_CHARS: usize = 8;

/// A game's join code. It never appears in `Debug` output or logs.
#[derive(Clone, PartialEq, Eq)]
pub struct JoinCode(String);

impl JoinCode {
    /// A fresh code from the operating system's CSPRNG.
    pub fn generate() -> Result<JoinCode, RandomError> {
        let mut bytes = [0u8; CODE_CHARS];
        getrandom::fill(&mut bytes).map_err(|_| RandomError)?;
        // 256 is a multiple of 32, so the low five bits of a random byte are uniform.
        let text = bytes
            .iter()
            .map(|byte| char::from(ALPHABET[usize::from(byte & 31)]))
            .collect();
        Ok(JoinCode(text))
    }

    /// The code a person typed: case and spaces or hyphens between groups do not matter. `None` if
    /// what is left is not eight symbols of the alphabet, so it can never be a code.
    pub fn parse(typed: &str) -> Option<JoinCode> {
        let text: String = typed
            .trim()
            .chars()
            .filter(|c| !matches!(c, ' ' | '-'))
            .map(|c| c.to_ascii_uppercase())
            .collect();
        let valid = text.len() == CODE_CHARS && text.bytes().all(|b| ALPHABET.contains(&b));
        valid.then_some(JoinCode(text))
    }

    /// The code text, for the one response that shows it to the creator.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// The digest the registry and the database keep in place of the code.
    pub fn digest(&self) -> TokenDigest {
        TokenDigest::of(&self.0)
    }
}

impl fmt::Debug for JoinCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JoinCode(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a join code is eight symbols of a 32-symbol alphabet without look-alikes, drawn uniformly and without repeats, typed forgivingly, never printed, and kept as the SHA-256 of its canonical text.
    use std::collections::{BTreeMap, HashSet};
    use std::fmt::Write;

    use super::*;
    use rstest::rstest;

    #[test]
    fn the_alphabet_has_32_distinct_capitals_and_digits_without_look_alikes() {
        let distinct: HashSet<u8> = ALPHABET.iter().copied().collect();
        assert_eq!(distinct.len(), 32);
        assert!(
            ALPHABET
                .iter()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        );
        for look_alike in b"IO01" {
            assert!(
                !ALPHABET.contains(look_alike),
                "{}",
                char::from(*look_alike)
            );
        }
        // 40 bits: eight symbols of five bits each.
        assert_eq!(CODE_CHARS * 5, 40);
    }

    #[test]
    fn generated_codes_have_eight_symbols_of_the_alphabet() {
        for _ in 0..500 {
            let code = JoinCode::generate().unwrap();
            let text = code.expose();
            assert_eq!(text.len(), CODE_CHARS);
            assert!(text.bytes().all(|b| ALPHABET.contains(&b)), "{text}");
            assert_eq!(JoinCode::parse(text), Some(code));
        }
    }

    #[test]
    fn many_generated_codes_are_distinct() {
        // 40 bits: three thousand draws collide about once in 250 000 runs.
        let codes: Vec<JoinCode> = (0..3_000).map(|_| JoinCode::generate().unwrap()).collect();
        let distinct: HashSet<&str> = codes.iter().map(JoinCode::expose).collect();
        assert_eq!(distinct.len(), codes.len());
        let digests: HashSet<[u8; 32]> = codes.iter().map(|c| *c.digest().as_bytes()).collect();
        assert_eq!(digests.len(), codes.len());
    }

    #[test]
    fn every_symbol_turns_up_about_as_often_as_the_others() {
        let mut seen: BTreeMap<char, usize> = BTreeMap::new();
        for _ in 0..3_000 {
            for c in JoinCode::generate().unwrap().expose().chars() {
                *seen.entry(c).or_default() += 1;
            }
        }
        // 24 000 draws over 32 symbols: 750 each, a standard deviation of about 27.
        assert_eq!(seen.len(), 32);
        for (symbol, count) in seen {
            assert!(
                (550..950).contains(&count),
                "{symbol} came up {count} times"
            );
        }
    }

    #[rstest]
    #[case::as_shown("ABCD2345", Some("ABCD2345"))]
    #[case::lower_case("abcd2345", Some("ABCD2345"))]
    #[case::mixed_case("aBcD2345", Some("ABCD2345"))]
    #[case::hyphen("ABCD-2345", Some("ABCD2345"))]
    #[case::space("abcd 2345", Some("ABCD2345"))]
    #[case::padded("  ABCD2345\n", Some("ABCD2345"))]
    #[case::several_separators("A-B C-D 2345", Some("ABCD2345"))]
    #[case::empty("", None)]
    #[case::too_short("ABCD234", None)]
    #[case::too_long("ABCD23456", None)]
    #[case::letter_i("ABCDI345", None)]
    #[case::letter_o("ABCDO345", None)]
    #[case::digit_zero("ABCD0345", None)]
    #[case::digit_one("ABCD1345", None)]
    #[case::punctuation("ABCD.345", None)]
    #[case::inner_newline("ABCD\n2345", None)]
    #[case::non_ascii("ÄBCD2345", None)]
    #[case::wide_letters("ＡＢＣＤ２３４５", None)]
    fn a_typed_code_is_normalised_or_refused(#[case] typed: &str, #[case] expected: Option<&str>) {
        let parsed = JoinCode::parse(typed);
        assert_eq!(parsed.as_ref().map(JoinCode::expose), expected);
    }

    #[test]
    fn the_digest_is_the_sha256_of_the_canonical_text() {
        let code = JoinCode::parse("abcd-efgh").unwrap();
        assert_eq!(code.expose(), "ABCDEFGH");
        let hex = code
            .digest()
            .as_bytes()
            .iter()
            .fold(String::new(), |mut out, byte| {
                write!(out, "{byte:02x}").unwrap();
                out
            });
        assert_eq!(
            hex,
            "9ac2197d9258257b1ae8463e4214e4cd0a578bc1517f2415928b91be4283fc48"
        );
        // However it was typed, the same code has the same digest, and another code another one.
        let typed = JoinCode::parse(" ab cd ef gh ").unwrap();
        assert!(typed.digest().matches(&code.digest()));
        assert!(
            !JoinCode::parse("ABCDEFGJ")
                .unwrap()
                .digest()
                .matches(&code.digest())
        );
    }

    #[test]
    fn debug_output_hides_the_code() {
        let code = JoinCode::generate().unwrap();
        let shown = format!("{code:?} {code:#?} {:?}", code.digest());
        assert!(!shown.contains(code.expose()), "{shown}");
    }
}
