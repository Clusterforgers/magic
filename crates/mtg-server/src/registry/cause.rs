//! Why a game failed to resume, in the sentence its players read.

/// What kept a stored game from coming back. The record keeps the operator's detail beside it;
/// players only ever see [`Cause::reason`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Cause {
    /// The saved setup cannot be read, or no longer makes a game.
    Setup,
    /// The cards of its decks cannot be had.
    Cards,
    /// A deck is not what it was when the game was made; `player` is who plays it.
    DeckChanged { player: String },
    /// The saved seats do not fit the setup.
    Seats,
    /// The saved moves cannot be loaded or replayed.
    Replay,
    /// The game began while a seat was empty.
    EmptySeat,
    /// The game was marked failed without a reason kept.
    Unknown,
}

impl Cause {
    /// One sentence for the players of the game: what happened, without internals.
    pub(crate) fn reason(&self) -> String {
        match self {
            Cause::Setup => {
                "The saved setup of this game cannot be read, so it cannot continue.".to_owned()
            }
            Cause::Cards => {
                "The cards of this game's decks are not available, so it cannot continue."
                    .to_owned()
            }
            Cause::DeckChanged { player } => {
                format!(
                    "The deck of {player} has changed since this game was made, so it cannot continue."
                )
            }
            Cause::Seats => {
                "The saved seats of this game do not fit its setup, so it cannot continue."
                    .to_owned()
            }
            Cause::Replay => {
                "The saved moves of this game cannot be loaded, so it cannot continue.".to_owned()
            }
            Cause::EmptySeat => {
                "This game began with a seat nobody holds, so it cannot continue.".to_owned()
            }
            Cause::Unknown => "This game could not be restored, so it cannot continue.".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every cause reads as one plain sentence that says the game cannot continue and carries nothing of the server's insides, and a deck change names the player.
    use rstest::rstest;

    use super::*;

    fn player() -> Cause {
        Cause::DeckChanged {
            player: "Ann".to_owned(),
        }
    }

    #[rstest]
    #[case::setup(Cause::Setup)]
    #[case::cards(Cause::Cards)]
    #[case::deck(player())]
    #[case::seats(Cause::Seats)]
    #[case::replay(Cause::Replay)]
    #[case::empty(Cause::EmptySeat)]
    #[case::unknown(Cause::Unknown)]
    fn a_reason_is_a_plain_sentence_without_internals(#[case] cause: Cause) {
        let reason = cause.reason();
        assert!(reason.ends_with(" so it cannot continue."), "{reason}");
        assert!(reason.starts_with(|c: char| c.is_uppercase()), "{reason}");
        assert_eq!(reason.matches('.').count(), 1, "{reason}");
        for internal in [
            "recipe",
            "fingerprint",
            "oracle",
            "sql",
            "database",
            "panic",
            "error",
            "{",
            ":",
            "uuid",
            "v1",
        ] {
            assert!(!reason.to_lowercase().contains(internal), "{reason}");
        }
        assert!(!reason.chars().any(|c| c.is_ascii_digit()), "{reason}");
    }

    #[test]
    fn a_changed_deck_names_whose_it_is() {
        assert!(player().reason().contains("The deck of Ann has changed"));
    }
}
