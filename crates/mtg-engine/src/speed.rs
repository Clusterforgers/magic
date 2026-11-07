//! Speed (CR 702.179): a number from 0 to 4 each player has, which start your engines! sets to 1 and which rises once
//! on each of its player's turns when an opponent loses life.
//!
//! A player who controls a permanent with start your engines! and has no speed gets speed 1 with the state-based
//! actions. A player with speed has the inherent trigger "Whenever one or more opponents lose life during your turn, if
//! your speed is less than 4, increase your speed by 1. This ability triggers only once each turn." (CR 702.179d): the
//! life loss marks the rise as due while it folds, and the state-based actions give it, so like the monarch's draw it
//! does not wait on the stack. Speed stays when the permanent leaves.

use std::collections::BTreeMap;

use mtg_cards::keyword::Keyword;
use mtg_core::ids::PlayerId;

use crate::characteristics::controller_of_object;
use crate::events::Event;
use crate::keywords::has_keyword;
use crate::state::GameState;
use crate::zones::Zone;

/// The highest speed (CR 702.179b): max speed.
pub const MAX_SPEED: u8 = 4;

/// Each player's speed and the rise the turn owes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(crate) struct Speeds {
    /// The speed of each player who has one.
    levels: BTreeMap<PlayerId, u8>,
    /// The turn whose speed trigger has triggered: it triggers once each turn.
    triggered_on: Option<u32>,
    /// The active player whose speed rises at the next state-based check.
    due: Option<PlayerId>,
}

impl Speeds {
    /// Whether no player has speed: nothing to hash or watch.
    pub(crate) fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }
}

impl GameState {
    /// The player's speed: 0 while they have none.
    pub fn speed(&self, player: PlayerId) -> u8 {
        self.speeds.levels.get(&player).copied().unwrap_or(0)
    }

    /// Whether the player has max speed.
    pub fn has_max_speed(&self, player: PlayerId) -> bool {
        self.speed(player) >= MAX_SPEED
    }
}

/// Folds a player's new speed; a rise that was due is given.
pub(crate) fn set(state: &mut GameState, player: PlayerId, speed: u8) {
    state.speeds.levels.insert(player, speed.min(MAX_SPEED));
    if state.speeds.due == Some(player) {
        state.speeds.due = None;
    }
}

/// Marks the rise of the active player's speed as due when `player` losing `amount` life triggers it: an opponent of
/// the active player lost life on their turn, the active player has speed below 4, and it has not triggered this turn.
pub(crate) fn on_life_lost(state: &mut GameState, player: PlayerId, amount: u32) {
    let active = state.turn.active;
    let turn = state.turn.number;
    let speed = state.speed(active);
    if amount == 0
        || player == active
        || speed == 0
        || speed >= MAX_SPEED
        || state.speeds.triggered_on == Some(turn)
    {
        return;
    }
    state.speeds.triggered_on = Some(turn);
    state.speeds.due = Some(active);
}

/// What one pass of the state-based actions gives: speed 1 to each player with none who controls a permanent with
/// start your engines!, and the rise that is due.
pub(crate) fn sync(state: &GameState) -> Vec<Event> {
    let mut events = Vec::new();
    if let Some(player) = state.speeds.due
        && state.is_alive(player)
    {
        let speed = (state.speed(player) + 1).min(MAX_SPEED);
        events.push(Event::SpeedChanged { player, speed });
    }
    if !state.possible.engines {
        return events;
    }
    let mut starting: Vec<PlayerId> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| has_keyword(state, **id, &Keyword::StartYourEngines))
        .filter_map(|id| state.object(*id))
        .map(|object| controller_of_object(state, object))
        .filter(|player| state.speed(*player) == 0 && state.is_alive(*player))
        .collect();
    starting.sort_unstable();
    starting.dedup();
    events.extend(
        starting
            .into_iter()
            .map(|player| Event::SpeedChanged { player, speed: 1 }),
    );
    events
}

#[cfg(test)]
mod tests {
    //! Goal: start your engines! gives its controller speed 1 once; an opponent's life loss on your turn raises your speed once that turn and never past 4; your own loss, a loss on another turn and a player without speed raise nothing.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn racer() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Racer")
                .with_type(mtg_core::card::CardType::Creature)
                .with_power_toughness(1, 1)
                .with_rules(Ability::Keyword(Keyword::StartYourEngines)),
        )
    }

    /// Folds every state-based speed event until none is left.
    fn settle(game: &mut GameState) {
        for _ in 0..8 {
            let events = sync(game);
            if events.is_empty() {
                return;
            }
            for event in &events {
                apply_event(game, event).unwrap();
            }
        }
    }

    fn lose(game: &mut GameState, player: PlayerId, amount: u32) {
        apply_event(game, &Event::LifeLost { player, amount }).unwrap();
        settle(game);
    }

    #[test]
    fn start_your_engines_gives_speed_one_once() {
        let mut game = state(vec![vec![], vec![]], vec![vec![racer(), racer()], vec![]]);
        assert_eq!(sync(&game), [Event::SpeedChanged { player: ME, speed: 1 }]);
        settle(&mut game);
        assert_eq!((game.speed(ME), game.speed(FOE)), (1, 0));
        assert_eq!(sync(&game), []);
    }

    #[test]
    fn a_permanent_without_the_keyword_gives_no_speed() {
        let game = state(vec![vec![], vec![]], vec![vec![creature("Bear", 2, 2)], vec![]]);
        assert_eq!(sync(&game), []);
    }

    #[rstest]
    #[case::an_opponent_loses_life(1, FOE, 1, 2)]
    #[case::your_own_loss(1, ME, 3, 1)]
    #[case::no_speed_rises_nothing(0, FOE, 1, 0)]
    #[case::max_speed_stays(4, FOE, 1, 4)]
    fn a_loss_on_your_turn_raises_your_speed(
        #[case] start: u8,
        #[case] loser: PlayerId,
        #[case] amount: u32,
        #[case] expected: u8,
    ) {
        let mut game = state(vec![vec![], vec![]], vec![vec![], vec![]]);
        if start > 0 {
            set(&mut game, ME, start);
        }
        lose(&mut game, loser, amount);
        assert_eq!(game.speed(ME), expected);
    }

    #[test]
    fn the_rise_triggers_once_each_turn() {
        let mut game = state(vec![vec![], vec![]], vec![vec![], vec![]]);
        set(&mut game, ME, 1);
        lose(&mut game, FOE, 1);
        lose(&mut game, FOE, 2);
        assert_eq!(game.speed(ME), 2);
        game.turn.number += 1;
        lose(&mut game, FOE, 1);
        assert_eq!(game.speed(ME), 3);
    }

    #[test]
    fn a_loss_on_an_opponents_turn_raises_nothing() {
        let mut game = state(vec![vec![], vec![]], vec![vec![], vec![]]);
        set(&mut game, FOE, 1);
        lose(&mut game, ME, 1);
        assert_eq!(game.speed(FOE), 1);
    }

    #[test]
    fn speed_gained_after_the_loss_does_not_rise_that_turn() {
        let mut game = state(vec![vec![], vec![]], vec![vec![racer()], vec![]]);
        apply_event(&mut game, &Event::LifeLost { player: FOE, amount: 2 }).unwrap();
        settle(&mut game);
        assert_eq!(game.speed(ME), 1);
    }
}
