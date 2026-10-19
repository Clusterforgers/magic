//! The replacement step: a stop in front of the fold for the events that "would" happen (CR 614).
//!
//! Effect steps and rules code propose events and [`process`] gives back the events that happen. It is the
//! seam for replacement and prevention effects and does nothing yet: every proposal passes unchanged. When
//! several effects apply, the affected player orders them with a `ChooseReplacement` decision whose answer is
//! a journal entry, so `process` stays a pure function of state and journal (`docs/cards-dsl.md`).

use crate::decision::Decision;
use crate::events::Event;
use crate::state::GameState;

/// What the replacement step made of the proposed events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Processed {
    /// The events that happen.
    Events(Vec<Event>),
    /// Several replacement effects apply and a player has to order them first.
    Ask(Decision),
}

/// Passes the proposed events through the replacement effects that apply to them in `state`.
pub fn process(_state: &GameState, proposed: Vec<Event>) -> Processed {
    Processed::Events(proposed)
}

#[cfg(test)]
mod tests {
    //! Goal: with no replacement effects in the engine, the step hands back exactly the events it was given, in order, whatever they are.
    use super::*;
    use crate::choose::Choices;
    use crate::combat::DamageTarget;
    use mtg_core::ids::{ObjectId, PlayerId};

    fn state() -> GameState {
        let setup = crate::setup::GameSetup::new(
            std::sync::Arc::new(crate::format::Normal),
            1,
            vec![Vec::new(), Vec::new()],
        );
        GameState::from_setup(&GameSetup {
            validate_decks: false,
            ..setup
        })
        .unwrap()
    }

    use crate::setup::GameSetup;

    #[test]
    fn every_proposal_passes_unchanged() {
        let proposed = vec![
            Event::DamageDealt {
                source: ObjectId(1),
                target: DamageTarget::Player(PlayerId(0)),
                amount: 3,
            },
            Event::LifeLost {
                player: PlayerId(1),
                amount: 2,
            },
            Event::SpellCast {
                player: PlayerId(0),
                object: ObjectId(2),
                choices: Choices::default(),
            },
        ];
        assert_eq!(
            process(&state(), proposed.clone()),
            Processed::Events(proposed)
        );
        assert_eq!(process(&state(), Vec::new()), Processed::Events(Vec::new()));
    }
}
