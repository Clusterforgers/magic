//! Crimes (CR 700.13): a player commits a crime when they cast a spell, activate an ability or put a triggered ability
//! on the stack that targets an opponent, an object an opponent controls or a card in an opponent's graveyard; one
//! spell or ability is one crime however many targets it has.

use mtg_core::ids::PlayerId;

use crate::characteristics::controller_of_object;
use crate::choose::{Choices, Target};
use crate::state::GameState;
use crate::zones::Zone;

/// Whether the spell or ability `player` announced with `choices` is a crime against an opponent.
pub(super) fn w11c_is_crime(state: &GameState, player: PlayerId, choices: &Choices) -> bool {
    choices.targets.iter().flatten().any(|target| match target {
        Target::Player(other) => *other != player,
        Target::Object(object) => state.object(object.id).is_some_and(|found| {
            let whose = match found.zone() {
                Zone::Graveyard(_) => found.owner(),
                Zone::Battlefield | Zone::Stack => controller_of_object(state, found),
                _ => return false,
            };
            whose != player
        }),
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a spell or ability is a crime when one of its targets is an opponent, a permanent an opponent controls or
    //! a card in an opponent's graveyard, and "whenever you commit a crime" and "whenever an opponent commits a crime"
    //! trigger once for each crime of the players they name.
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::super::detect_events::{FOE, ME, board, fired, fold, id, target};
    use crate::choose::{Choices, Target};
    use crate::events::Event;
    use crate::events::ZoneCause;
    use crate::state::GameState;
    use crate::zones::Zone;

    /// A cast of `name` by `player` with the targets named: a player by "me" or "foe", an object by its name.
    fn w11c_cast(state: &GameState, player: PlayerId, name: &str, targets: &[&str]) -> Event {
        let chosen = targets
            .iter()
            .map(|t| match *t {
                "me" => Target::Player(ME),
                "foe" => Target::Player(FOE),
                object => target(state, object),
            })
            .collect();
        Event::SpellCast {
            player,
            object: id(state, name),
            choices: Choices {
                targets: vec![chosen],
                ..Choices::default()
            },
        }
    }

    #[rstest]
    #[case::an_opponent(&["foe"], 1)]
    #[case::myself(&["me"], 0)]
    #[case::their_creature(&["Ogre"], 1)]
    #[case::my_creature(&["Bear"], 0)]
    #[case::their_land(&["Their Forest"], 1)]
    #[case::two_of_theirs_one_crime(&["Ogre", "Wolf"], 1)]
    #[case::mine_and_theirs(&["Bear", "foe"], 1)]
    #[case::no_target(&[], 0)]
    fn w11c_a_crime_is_a_target_of_an_opponent(#[case] targets: &[&str], #[case] expected: usize) {
        let pre = board(r#"{"W11cCrime":{"who":"You"}}"#);
        let cast = w11c_cast(&pre, ME, "Spell", targets);
        assert_eq!(fired(&pre, &[cast]), expected);
    }

    #[rstest]
    #[case::mine_by_me(r#"{"W11cCrime":{"who":"You"}}"#, ME, 1)]
    #[case::mine_by_them(r#"{"W11cCrime":{"who":"You"}}"#, FOE, 0)]
    #[case::an_opponents(r#"{"W11cCrime":{"who":"Opponents"}}"#, FOE, 1)]
    #[case::not_an_opponents(r#"{"W11cCrime":{"who":"Opponents"}}"#, ME, 0)]
    #[case::anyones(r#"{"W11cCrime":{}}"#, FOE, 1)]
    fn w11c_crime_names_the_players_of_the_pattern(
        #[case] when: &str,
        #[case] player: PlayerId,
        #[case] expected: usize,
    ) {
        let pre = board(when);
        let (name, victim) = if player == ME {
            ("Spell", "Ogre")
        } else {
            ("Their Spell", "Bear")
        };
        let cast = w11c_cast(&pre, player, name, &[victim]);
        assert_eq!(fired(&pre, &[cast]), expected);
    }

    #[test]
    fn w11c_a_card_in_an_opponents_graveyard_is_a_crime_and_one_in_mine_is_not() {
        let start = board(r#"{"W11cCrime":{"who":"You"}}"#);
        let died: Vec<Event> = [("Ogre", FOE), ("Bear", ME)]
            .into_iter()
            .map(|(name, owner)| Event::ZoneChanged {
                object: id(&start, name),
                from: Zone::Battlefield,
                to: Zone::Graveyard(owner),
                cause: ZoneCause::Moved,
            })
            .collect();
        let pre = fold(&start, &died);
        let theirs = w11c_cast(&pre, ME, "Spell", &["Ogre"]);
        assert_eq!(fired(&pre, &[theirs]), 1);
        let mine = w11c_cast(&pre, ME, "Spell", &["Bear"]);
        assert_eq!(fired(&pre, &[mine]), 0);
    }
}
