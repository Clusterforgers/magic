//! Dilemmas: a player faces a dilemma as they choose one or more modes for a spell or ability (CR 700.2), as it is cast,
//! activated or put on the stack. The player who chose the modes faces it ("an opponent chooses one"), and a pick of
//! modes at random is no choice of theirs; a copy keeps the modes of its original (CR 707.10) and faces none.

use mtg_core::ids::PlayerId;

use crate::choose::Choices;
use crate::events::Event;
use crate::state::{GameState, Tally};

/// The player who faces a dilemma with the spell or ability `controller` announced with `choices`, if one does.
pub(super) fn r116_dilemma_of(controller: PlayerId, choices: &Choices) -> Option<PlayerId> {
    (!choices.modes.is_empty() && !choices.random_modes)
        .then(|| choices.modes_chooser().unwrap_or(controller))
}

/// A spell cast or an ability put on the stack with modes chosen counts one dilemma for the player who chose them.
pub(crate) fn r116_note_dilemma(state: &mut GameState, event: &Event) {
    let (controller, choices) = match event {
        Event::SpellCast {
            player, choices, ..
        } => (*player, choices),
        Event::AbilityPutOnStack(put) => (put.controller, &put.choices),
        _ => return,
    };
    if let Some(player) = r116_dilemma_of(controller, choices) {
        state.tally(player, Tally::R116Dilemmas);
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a spell or ability announced with one or more modes chosen is a dilemma for the player who chose them, a
    //! random pick and an announcement without modes are none, and "whenever you face a dilemma" and "whenever an
    //! opponent faces a dilemma" trigger once for each dilemma of the players they name.
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::super::detect_events::{FOE, ME, board, fired, id};
    use crate::choose::Choices;
    use crate::events::Event;
    use crate::state::{GameState, Tally};

    /// A cast of `name` by `player` with these modes, chosen by `by` when that is another seat.
    fn r116_cast(
        state: &GameState,
        player: PlayerId,
        name: &str,
        modes: &[usize],
        by: Option<u8>,
        random: bool,
    ) -> Event {
        Event::SpellCast {
            player,
            object: id(state, name),
            choices: Choices {
                modes: modes.to_vec(),
                modes_by: by,
                random_modes: random,
                ..Choices::default()
            },
        }
    }

    #[rstest]
    #[case::one_mode(&[0], None, false, 1)]
    #[case::two_modes(&[0, 1], None, false, 1)]
    #[case::no_modes(&[], None, false, 0)]
    #[case::at_random(&[1], None, true, 0)]
    #[case::an_opponent_chose(&[0], Some(1), false, 0)]
    fn r116_a_dilemma_is_a_choice_of_modes(
        #[case] modes: &[usize],
        #[case] by: Option<u8>,
        #[case] random: bool,
        #[case] expected: usize,
    ) {
        let pre = board(r#"{"R116Dilemma":{"who":"You"}}"#);
        let cast = r116_cast(&pre, ME, "Spell", modes, by, random);
        assert_eq!(fired(&pre, &[cast]), expected);
    }

    #[rstest]
    #[case::mine_by_me(r#"{"R116Dilemma":{"who":"You"}}"#, ME, 1)]
    #[case::mine_by_them(r#"{"R116Dilemma":{"who":"You"}}"#, FOE, 0)]
    #[case::an_opponents(r#"{"R116Dilemma":{"who":"Opponents"}}"#, FOE, 1)]
    #[case::not_an_opponents(r#"{"R116Dilemma":{"who":"Opponents"}}"#, ME, 0)]
    #[case::anyones(r#"{"R116Dilemma":{}}"#, FOE, 1)]
    fn r116_dilemma_names_the_players_of_the_pattern(
        #[case] when: &str,
        #[case] player: PlayerId,
        #[case] expected: usize,
    ) {
        let pre = board(when);
        let name = if player == ME { "Spell" } else { "Their Spell" };
        let cast = r116_cast(&pre, player, name, &[0], None, false);
        assert_eq!(fired(&pre, &[cast]), expected);
    }

    #[test]
    fn r116_the_chooser_of_an_opponents_spell_faces_the_dilemma() {
        let pre = board(r#"{"R116Dilemma":{"who":"You"}}"#);
        // The opponent casts it and I choose its modes ("an opponent chooses one").
        let cast = r116_cast(&pre, FOE, "Their Spell", &[0], Some(0), false);
        assert_eq!(fired(&pre, std::slice::from_ref(&cast)), 1);
        let mut state = pre.clone();
        super::r116_note_dilemma(&mut state, &cast);
        assert_eq!(state.tallied(ME, Tally::R116Dilemmas), 1);
        assert_eq!(state.tallied(FOE, Tally::R116Dilemmas), 0);
    }
}
