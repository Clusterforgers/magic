//! The identifier of one account.

use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

/// Identifies one account across the database, the registry and the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UserId(pub Uuid);

impl UserId {
    /// A fresh random id (uuid v4, drawn from the operating system's CSPRNG).
    pub fn random() -> UserId {
        UserId(Uuid::new_v4())
    }

    /// The underlying uuid.
    pub const fn uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl FromStr for UserId {
    type Err = uuid::Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(text).map(UserId)
    }
}

impl From<Uuid> for UserId {
    fn from(uuid: Uuid) -> Self {
        UserId(uuid)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: an account id survives the text form the wire carries and random ids do not repeat.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(0)]
    #[case(1)]
    #[case(0xdead_beef_0000_0000_0000_0000_cafe_f00d)]
    #[case(u128::MAX)]
    fn text_form_round_trips(#[case] raw: u128) {
        let id = UserId(Uuid::from_u128(raw));
        assert_eq!(id.to_string().parse::<UserId>(), Ok(id));
        assert_eq!(UserId::from(id.uuid()), id);
    }

    #[test]
    fn random_ids_are_distinct() {
        let ids: std::collections::HashSet<UserId> = (0..64).map(|_| UserId::random()).collect();
        assert_eq!(ids.len(), 64);
    }

    #[rstest]
    #[case("")]
    #[case("alice")]
    #[case("0000000000000000000000000000000g")]
    fn bad_text_is_rejected(#[case] text: &str) {
        assert!(text.parse::<UserId>().is_err());
    }
}
