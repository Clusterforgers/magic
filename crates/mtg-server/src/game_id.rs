//! The identifier of one game.

use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

/// Identifies one game across the actor, the store and the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameId(pub Uuid);

impl GameId {
    /// A fresh random id (uuid v4, drawn from the operating system's CSPRNG).
    pub fn random() -> GameId {
        GameId(Uuid::new_v4())
    }

    /// The underlying uuid.
    pub const fn uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for GameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl FromStr for GameId {
    type Err = uuid::Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(text).map(GameId)
    }
}

impl From<Uuid> for GameId {
    fn from(uuid: Uuid) -> Self {
        GameId(uuid)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a game id survives the text form that rides in `ClientMessage::Join`.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(0)]
    #[case(1)]
    #[case(0xdead_beef_0000_0000_0000_0000_cafe_f00d)]
    #[case(u128::MAX)]
    fn text_form_round_trips(#[case] raw: u128) {
        let id = GameId(Uuid::from_u128(raw));
        assert_eq!(id.to_string().parse::<GameId>(), Ok(id));
        assert_eq!(GameId::from(id.uuid()), id);
    }

    #[test]
    fn random_ids_are_distinct_and_survive_the_text_form() {
        let ids: Vec<GameId> = (0..64).map(|_| GameId::random()).collect();
        for id in &ids {
            assert_eq!(id.to_string().parse::<GameId>(), Ok(*id));
        }
        let distinct: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(distinct.len(), ids.len());
    }

    #[rstest]
    #[case("")]
    #[case("not-a-uuid")]
    #[case("0000000000000000000000000000000g")]
    fn bad_text_is_rejected(#[case] text: &str) {
        assert!(text.parse::<GameId>().is_err());
    }
}
