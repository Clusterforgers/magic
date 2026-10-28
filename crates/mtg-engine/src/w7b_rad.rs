//! Rad counters (CR 725): the mill, life loss and removal of rad counters as the precombat main phase begins.
//!
//! Rad counters are counters on a player (`CounterKind::Other("rad")`). As the active player's precombat main phase
//! begins, that player mills a card for each rad counter they have, then loses 1 life and removes a rad counter for
//! each nonland card milled this way (CR 725.2). The engine does it as the step begins, like the monarch's draw, so it
//! never waits on the stack: the log shows the milled cards (`ZoneCause::Milled`), the `LifeLost` and the
//! `CountersChanged` that follow.

use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;

use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::{Event, ZoneCause};
use crate::library::top_cards;
use crate::rules::life_total_cant_change;
use crate::state::GameState;
use crate::zones::Zone;

/// The rad counter kind.
pub(crate) fn w7b_rad() -> CounterKind {
    CounterKind::Other("rad".to_owned())
}

/// The events of the rad rule for the active player as their precombat main phase begins: none without rad counters.
pub(crate) fn w7b_precombat_rad(state: &GameState) -> Vec<Event> {
    state
        .active_player()
        .map_or_else(Vec::new, |active| w7b_rad_events(state, active))
}

/// `player` mills a card per rad counter, then loses 1 life and removes a rad counter for each nonland card milled.
fn w7b_rad_events(state: &GameState, player: PlayerId) -> Vec<Event> {
    let rad = state
        .player(player)
        .map_or(0, |p| p.counters_of(&w7b_rad()));
    if rad == 0 || !state.is_alive(player) {
        return Vec::new();
    }
    let milled = top_cards(state, player, usize::try_from(rad).unwrap_or(usize::MAX));
    let nonland = milled
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .is_some_and(|object| !object.card().types.contains(&CardType::Land))
        })
        .count();
    let mut events: Vec<Event> = milled
        .into_iter()
        .map(|object| Event::ZoneChanged {
            object,
            from: Zone::Library(player),
            to: Zone::Graveyard(player),
            cause: ZoneCause::Milled,
        })
        .collect();
    let nonland = u32::try_from(nonland).unwrap_or(u32::MAX);
    if nonland == 0 {
        return events;
    }
    if !life_total_cant_change(state, player) {
        events.push(Event::LifeLost {
            player,
            amount: nonland,
        });
    }
    events.push(Event::CountersChanged(CountersChanged {
        on: Target::Player(player),
        counter: w7b_rad(),
        delta: -i32::try_from(nonland).unwrap_or(i32::MAX),
    }));
    events
}

#[cfg(test)]
mod tests {
    //! Goal: as the precombat main phase begins, the active player mills a card per rad counter, then loses 1 life and removes a rad counter per nonland card milled; lands milled cost nothing, a short library gives what it has, a player without rad counters does nothing, and only the active player's counters count.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};

    const ME: PlayerId = PlayerId(0);

    fn land(name: &str) -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new(name).with_type(CardType::Land))
    }

    /// Seat 0 holds `rad` rad counters over a library whose top cards are `top` ('L' a land, 'C' a creature), the
    /// first letter on top; seat 1 holds five rad counters.
    fn game(rad: u32, top: &str) -> GameState {
        let mut setup = setup(vec![Vec::new(); 2], vec![Vec::new(); 2]);
        setup.players[0].library = top
            .chars()
            .enumerate()
            .map(|(n, c)| match c {
                'L' => land(&format!("Land {n}")),
                _ => creature(&format!("Bear {n}"), 2, 2),
            })
            .collect();
        setup.players[0].counters = vec![(w7b_rad(), rad)];
        setup.players[1].counters = vec![(w7b_rad(), 5)];
        GameState::from_setup(&setup).unwrap()
    }

    /// (cards milled, life lost, rad counters removed).
    fn outcome(events: &[Event]) -> (usize, u32, i32) {
        let milled = events
            .iter()
            .filter(|e| matches!(e, Event::ZoneChanged { cause: ZoneCause::Milled, from: Zone::Library(p), .. } if *p == ME))
            .count();
        let lost = events
            .iter()
            .map(|e| match e {
                Event::LifeLost { player, amount } if *player == ME => *amount,
                _ => 0,
            })
            .sum();
        let removed = events
            .iter()
            .map(|e| match e {
                Event::CountersChanged(c) if c.on == Target::Player(ME) => -c.delta,
                _ => 0,
            })
            .sum();
        (milled, lost, removed)
    }

    #[rstest]
    #[case::no_rad_counters(0, "CCC", (0, 0, 0))]
    #[case::all_nonland(3, "CCCC", (3, 3, 3))]
    #[case::lands_cost_nothing(3, "LCLC", (3, 1, 1))]
    #[case::only_lands(2, "LLC", (2, 0, 0))]
    #[case::short_library(4, "CL", (2, 1, 1))]
    #[case::empty_library(2, "", (0, 0, 0))]
    fn the_rad_rule_mills_and_counts_the_nonland_cards(
        #[case] rad: u32,
        #[case] top: &str,
        #[case] expected: (usize, u32, i32),
    ) {
        let game = game(rad, top);
        let events = w7b_precombat_rad(&game);
        assert_eq!(outcome(&events), expected);
        let others = events.iter().any(|e| match e {
            Event::CountersChanged(c) => c.on != Target::Player(ME),
            Event::LifeLost { player, .. } => *player != ME,
            _ => false,
        });
        assert!(!others, "only the active player's rad counters count");
    }

    #[test]
    fn the_cards_milled_are_the_top_ones_in_order() {
        let game = game(2, "CLC");
        let library = game.zone(Zone::Library(ME)).to_vec();
        let moved: Vec<_> = w7b_precombat_rad(&game)
            .iter()
            .filter_map(|e| match e {
                Event::ZoneChanged { object, .. } => Some(*object),
                _ => None,
            })
            .collect();
        assert_eq!(
            moved,
            library.iter().rev().take(2).copied().collect::<Vec<_>>()
        );
    }
}
