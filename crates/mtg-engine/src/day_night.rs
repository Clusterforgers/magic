//! Day and night (CR 726) and the daybound and nightbound permanents that follow it (CR 702.145).
//!
//! A game is neither day nor night until a permanent with daybound or nightbound is on the battlefield; then it becomes
//! day (night for nightbound alone). At the start of each turn it changes by the spells the previous turn's active player
//! cast: day to night after none, night to day after two or more (CR 726.3a). A daybound permanent is turned to its night
//! face while it is night and a nightbound one to its day face while it is day, as state-based actions are checked.

use mtg_cards::definition::Layout;
use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;
use serde::{Deserialize, Serialize};

use crate::events::Event;
use crate::keywords::has_keyword;
use crate::state::{GameState, Tally};
use crate::zones::Zone;

/// Whether it is day or night (CR 726.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum DayNight {
    /// It is day.
    Day,
    /// It is night.
    Night,
}

impl GameState {
    /// Day or night, or neither before a permanent with daybound or nightbound has appeared.
    pub const fn day_night(&self) -> Option<DayNight> {
        self.day_night
    }
}

/// What the turn change makes of the day or night (CR 726.3a): `state` is the game as the turn ends, so the spells
/// counted are the ending turn's and its active player the one whose casts count.
pub(crate) fn at_turn_start(state: &GameState) -> Option<DayNight> {
    let cast = state.tallied(state.turn().active(), Tally::Cast);
    match state.day_night? {
        DayNight::Day if cast == 0 => Some(DayNight::Night),
        DayNight::Night if cast >= 2 => Some(DayNight::Day),
        _ => None,
    }
}

/// The face a transforming double-faced permanent is turned to: the back face if the front is up, else the front.
pub(crate) const fn turned_over(face: Option<u8>) -> Option<u8> {
    match face {
        None => Some(1),
        Some(_) => None,
    }
}

/// The permanents on the battlefield that have `keyword` now and can be turned over.
fn bound(state: &GameState, keyword: &Keyword) -> Vec<ObjectId> {
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            state
                .object(*id)
                .is_some_and(|object| object.whole_card().layout == Layout::Transform)
                && has_keyword(state, *id, keyword)
        })
        .collect()
}

/// What the state asks of day and night right now (one pass of the state-based actions): the first permanent with
/// daybound or nightbound makes it day or night; while it is day each nightbound permanent is turned to its day face,
/// while it is night each daybound one to its night face.
pub(crate) fn sync(state: &GameState) -> Vec<Event> {
    if !state.possible.day_night {
        return Vec::new();
    }
    let (day, night) = (
        || bound(state, &Keyword::Daybound),
        || bound(state, &Keyword::Nightbound),
    );
    match state.day_night {
        None if !day().is_empty() => vec![Event::DayNightChanged { to: DayNight::Day }],
        None if !night().is_empty() => vec![Event::DayNightChanged {
            to: DayNight::Night,
        }],
        None => Vec::new(),
        Some(DayNight::Day) => turn_over(state, night()),
        Some(DayNight::Night) => turn_over(state, day()),
    }
}

/// The events that turn each of the permanents over.
fn turn_over(state: &GameState, permanents: Vec<ObjectId>) -> Vec<Event> {
    permanents
        .into_iter()
        .filter_map(|object| {
            let face = state.object(object)?.face();
            Some(Event::Transformed {
                object,
                face: turned_over(face),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: the turn change moves day to night after a turn whose active player cast no spells and night to day after one that cast two or more, and leaves day, night and neither as they are otherwise (CR 726.3a); a face is turned to the other one and back.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::setup;

    #[rstest]
    #[case::neither_stays_neither(None, 0, None)]
    #[case::neither_ignores_spells(None, 3, None)]
    #[case::day_with_no_spell_is_night(Some(DayNight::Day), 0, Some(DayNight::Night))]
    #[case::day_with_one_spell_stays(Some(DayNight::Day), 1, None)]
    #[case::day_with_two_spells_stays(Some(DayNight::Day), 2, None)]
    #[case::night_with_no_spell_stays(Some(DayNight::Night), 0, None)]
    #[case::night_with_one_spell_stays(Some(DayNight::Night), 1, None)]
    #[case::night_with_two_spells_is_day(Some(DayNight::Night), 2, Some(DayNight::Day))]
    #[case::night_with_many_spells_is_day(Some(DayNight::Night), 5, Some(DayNight::Day))]
    fn the_turn_change_follows_the_spells_the_ending_turns_active_player_cast(
        #[case] now: Option<DayNight>,
        #[case] cast: u32,
        #[case] then: Option<DayNight>,
    ) {
        let mut game =
            GameState::from_setup(&setup(vec![vec![], vec![]], vec![vec![], vec![]])).unwrap();
        game.day_night = now;
        for _ in 0..cast {
            game.tally(PlayerId(0), Tally::Cast);
        }
        // The other player's spells do not count.
        game.tally(PlayerId(1), Tally::Cast);
        game.tally(PlayerId(1), Tally::Cast);
        assert_eq!(at_turn_start(&game), then);
    }

    #[rstest]
    #[case::front_to_back(None, Some(1))]
    #[case::back_to_front(Some(1), None)]
    fn a_face_is_turned_to_the_other_one(#[case] face: Option<u8>, #[case] turned: Option<u8>) {
        assert_eq!(turned_over(face), turned);
        assert_eq!(turned_over(turned), face);
    }
}
