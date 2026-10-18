//! The rules for usernames and passwords.
//!
//! A username is 3 to 32 ASCII letters, digits, `_` or `-`; the lower-case form is the unique key,
//! so `Ann` and `ann` are one name. A password is 10 to 128 characters and is never altered.

use mtg_protocol::rest::{
    PASSWORD_MAX_CHARS, PASSWORD_MIN_CHARS, USERNAME_MAX_CHARS, USERNAME_MIN_CHARS,
};

/// A username that follows the rules, in the two spellings the server uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Username {
    /// As typed: what other players see.
    pub(crate) display: String,
    /// Lower case: what uniqueness and login compare.
    pub(crate) key: String,
}

/// Which rule a username breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UsernameError {
    /// Fewer than [`USERNAME_MIN_CHARS`] characters.
    TooShort,
    /// More than [`USERNAME_MAX_CHARS`] characters.
    TooLong,
    /// A character other than an ASCII letter, a digit, `_` or `-`.
    BadCharacter,
}

/// Which rule a password breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PasswordError {
    /// Fewer than [`PASSWORD_MIN_CHARS`] characters.
    TooShort,
    /// More than [`PASSWORD_MAX_CHARS`] characters.
    TooLong,
}

impl Username {
    /// Checks `text` as typed; surrounding spaces are an error, not trimmed.
    pub(crate) fn parse(text: &str) -> Result<Username, UsernameError> {
        if !text.chars().all(is_username_char) {
            return Err(UsernameError::BadCharacter);
        }
        // Every character is ASCII now, so bytes and characters agree.
        if text.len() < USERNAME_MIN_CHARS {
            return Err(UsernameError::TooShort);
        }
        if text.len() > USERNAME_MAX_CHARS {
            return Err(UsernameError::TooLong);
        }
        Ok(Username {
            display: text.to_owned(),
            key: text.to_ascii_lowercase(),
        })
    }

    /// The sentence that tells a person the rules.
    pub(crate) fn rules() -> String {
        format!(
            "a username has {USERNAME_MIN_CHARS} to {USERNAME_MAX_CHARS} characters: letters, digits, _ and -"
        )
    }
}

/// Checks the length of `text` in characters; the password itself is not inspected further.
pub(crate) fn check_password(text: &str) -> Result<(), PasswordError> {
    // Counting stops one past the limit, so a huge body costs no more than a long one.
    let length = text.chars().take(PASSWORD_MAX_CHARS + 1).count();
    if length < PASSWORD_MIN_CHARS {
        Err(PasswordError::TooShort)
    } else if length > PASSWORD_MAX_CHARS {
        Err(PasswordError::TooLong)
    } else {
        Ok(())
    }
}

/// The sentence that tells a person the password rules.
pub(crate) fn password_rules() -> String {
    format!("a password has {PASSWORD_MIN_CHARS} to {PASSWORD_MAX_CHARS} characters")
}

fn is_username_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

#[cfg(test)]
mod tests {
    //! Goal: usernames and passwords are accepted exactly at their documented limits, every other shape is refused for the right reason, and a username's key ignores case.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::shortest("abc")]
    #[case::longest(&"a".repeat(USERNAME_MAX_CHARS))]
    #[case::digits_only("123")]
    #[case::underscore_and_dash("a_b-c")]
    #[case::leading_dash("-ab")]
    #[case::mixed_case("AnNa_Lee-9")]
    fn good_usernames(#[case] text: &str) {
        let name = Username::parse(text).unwrap();
        assert_eq!(name.display, text);
        assert_eq!(name.key, text.to_lowercase());
    }

    #[rstest]
    #[case::empty("", UsernameError::TooShort)]
    #[case::one("a", UsernameError::TooShort)]
    #[case::two("ab", UsernameError::TooShort)]
    #[case::one_too_long(&"a".repeat(USERNAME_MAX_CHARS + 1), UsernameError::TooLong)]
    #[case::very_long(&"a".repeat(5_000), UsernameError::TooLong)]
    #[case::space("an na", UsernameError::BadCharacter)]
    #[case::leading_space(" anna", UsernameError::BadCharacter)]
    #[case::trailing_space("anna ", UsernameError::BadCharacter)]
    #[case::newline("anna\n", UsernameError::BadCharacter)]
    #[case::tab("an\tna", UsernameError::BadCharacter)]
    #[case::nul("an\0na", UsernameError::BadCharacter)]
    #[case::dot("an.na", UsernameError::BadCharacter)]
    #[case::at_sign("anna@example", UsernameError::BadCharacter)]
    #[case::slash("an/na", UsernameError::BadCharacter)]
    #[case::accent("zoë", UsernameError::BadCharacter)]
    #[case::cyrillic_a("аnna", UsernameError::BadCharacter)]
    #[case::fullwidth("ａｂｃ", UsernameError::BadCharacter)]
    #[case::emoji("anna🙂", UsernameError::BadCharacter)]
    #[case::bidi_override("an\u{202e}na", UsernameError::BadCharacter)]
    #[case::zero_width("an\u{200b}na", UsernameError::BadCharacter)]
    #[case::long_and_bad(&format!("{}!", "a".repeat(40)), UsernameError::BadCharacter)]
    fn bad_usernames(#[case] text: &str, #[case] why: UsernameError) {
        assert_eq!(Username::parse(text), Err(why));
    }

    #[rstest]
    #[case("Anna", "anna")]
    #[case("ANNA", "anna")]
    #[case("aNnA_9-X", "anna_9-x")]
    fn the_key_ignores_case_and_the_display_does_not(#[case] text: &str, #[case] key: &str) {
        let name = Username::parse(text).unwrap();
        assert_eq!((name.display.as_str(), name.key.as_str()), (text, key));
    }

    #[test]
    fn names_that_differ_only_in_case_share_a_key() {
        let (a, b) = (
            Username::parse("Anna").unwrap(),
            Username::parse("aNNA").unwrap(),
        );
        assert_ne!(a.display, b.display);
        assert_eq!(a.key, b.key);
    }

    #[rstest]
    #[case::shortest(&"p".repeat(PASSWORD_MIN_CHARS), Ok(()))]
    #[case::longest(&"p".repeat(PASSWORD_MAX_CHARS), Ok(()))]
    #[case::spaces_count("          ", Ok(()))]
    #[case::unicode_counts_characters(&"é".repeat(PASSWORD_MIN_CHARS), Ok(()))]
    #[case::emoji_counts_characters(&"🙂".repeat(PASSWORD_MAX_CHARS), Ok(()))]
    #[case::with_nul("pass\0word12", Ok(()))]
    #[case::empty("", Err(PasswordError::TooShort))]
    #[case::one_short(&"p".repeat(PASSWORD_MIN_CHARS - 1), Err(PasswordError::TooShort))]
    #[case::short_unicode(&"é".repeat(PASSWORD_MIN_CHARS - 1), Err(PasswordError::TooShort))]
    #[case::one_long(&"p".repeat(PASSWORD_MAX_CHARS + 1), Err(PasswordError::TooLong))]
    #[case::one_long_unicode(&"🙂".repeat(PASSWORD_MAX_CHARS + 1), Err(PasswordError::TooLong))]
    #[case::huge(&"p".repeat(1 << 20), Err(PasswordError::TooLong))]
    fn password_lengths(#[case] text: &str, #[case] expected: Result<(), PasswordError>) {
        assert_eq!(check_password(text), expected);
    }

    #[test]
    fn the_rules_name_the_numbers() {
        let rules = Username::rules();
        assert!(rules.contains('3') && rules.contains("32"), "{rules}");
        let rules = password_rules();
        assert!(rules.contains("10") && rules.contains("128"), "{rules}");
    }
}
