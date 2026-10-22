//! The city's blessing (CR 702.131, 725): a designation a player keeps for the rest of the game once ascend gives it.
//!
//! A permanent with ascend gives its controller the blessing as soon as they control ten or more permanents, checked
//! with the state-based actions; an instant or sorcery spell with ascend does the same as it begins to resolve, before
//! any of its effects. Nothing takes the blessing away.

use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::controller_of_object;
use crate::events::Event;
use crate::keywords::has_keyword;
use crate::object::{GameObject, ObjectKind};
use crate::state::GameState;
use crate::zones::Zone;

/// How many permanents a player controls to get the blessing.
const PERMANENTS: usize = 10;

impl GameState {
    /// Whether the player has the city's blessing.
    pub fn has_citys_blessing(&self, player: PlayerId) -> bool {
        self.citys_blessing.contains(&player)
    }
}

/// The permanents `player` controls now.
fn permanents_of(state: &GameState, player: PlayerId) -> usize {
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| controller_of_object(state, object) == player)
        .count()
}

/// Whether `player` has no blessing yet and controls enough permanents for it.
fn earns(state: &GameState, player: PlayerId) -> bool {
    !state.has_citys_blessing(player) && permanents_of(state, player) >= PERMANENTS
}

/// What one pass of the state-based actions gives: the blessing to each player who controls a permanent with ascend and
/// ten or more permanents, and has none.
pub(crate) fn sync(state: &GameState) -> Vec<Event> {
    if !state.possible.ascend {
        return Vec::new();
    }
    let ascending: Vec<ObjectId> = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| has_keyword(state, *id, &Keyword::Ascend))
        .collect();
    state
        .alive_players()
        .filter(|player| {
            ascending.iter().any(|id| {
                state
                    .object(*id)
                    .is_some_and(|object| controller_of_object(state, object) == *player)
            }) && earns(state, *player)
        })
        .map(|player| Event::CitysBlessingGained { player })
        .collect()
}

/// The blessing an instant or sorcery spell with ascend gives its controller as it begins to resolve; none for another
/// object, a spell without ascend, or a controller who has it or does not control ten permanents.
pub(crate) fn on_resolving(state: &GameState, object: &GameObject) -> Option<Event> {
    let spell = object.kind() != ObjectKind::Ability && object.zone() == Zone::Stack;
    let player = object.controller();
    (state.possible.ascend
        && spell
        && has_keyword(state, object.id(), &Keyword::Ascend)
        && earns(state, player))
    .then_some(Event::CitysBlessingGained { player })
}

#[cfg(test)]
mod tests {
    //! Goal: a player with a permanent that has ascend and ten permanents gets the blessing once and keeps it when permanents leave; ten permanents without ascend, or ascend with nine, give nothing; the blessing is the player's alone; a spell with ascend gives it as it resolves.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn ascender() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Ascender")
                .with_type(mtg_core::card::CardType::Creature)
                .with_power_toughness(1, 1)
                .with_rules(Ability::Keyword(Keyword::Ascend)),
        )
    }

    /// Seat 0 controls `permanents` permanents, the first of which is an ascender when `ascending`.
    fn game(permanents: usize, ascending: bool) -> GameState {
        let mut board: Vec<_> = (0..permanents)
            .map(|n| creature(&format!("Bear {n}"), 2, 2))
            .collect();
        if ascending && !board.is_empty() {
            board[0] = ascender();
        }
        state(vec![vec![], vec![]], vec![board, vec![]])
    }

    #[rstest]
    #[case::ten_and_ascend(10, true, true)]
    #[case::more_than_ten(14, true, true)]
    #[case::nine_and_ascend(9, true, false)]
    #[case::ten_without_ascend(10, false, false)]
    fn ascend_gives_the_blessing_at_ten_permanents(
        #[case] permanents: usize,
        #[case] ascending: bool,
        #[case] gets: bool,
    ) {
        let game = game(permanents, ascending);
        let expected: Vec<Event> = gets
            .then_some(Event::CitysBlessingGained { player: ME })
            .into_iter()
            .collect();
        assert_eq!(sync(&game), expected);
    }

    #[test]
    fn the_blessing_is_kept_and_given_once() {
        let mut game = game(10, true);
        let gained = sync(&game);
        apply_event(&mut game, &gained[0]).unwrap();
        assert!(game.has_citys_blessing(ME));
        assert!(!game.has_citys_blessing(FOE));
        assert_eq!(sync(&game), []);
        // Folding the event again changes nothing: the designation is a set.
        apply_event(&mut game, &gained[0]).unwrap();
        assert!(game.has_citys_blessing(ME));
    }

    #[test]
    fn the_permanents_of_the_other_seat_do_not_count() {
        let board: Vec<_> = (0..10)
            .map(|n| creature(&format!("Bear {n}"), 2, 2))
            .collect();
        let game = state(vec![vec![], vec![]], vec![vec![ascender()], board]);
        assert_eq!(sync(&game), []);
    }
}
