//! Milling as a cost (CR 701.17): the top cards of the payer's library go to their graveyard.

use mtg_core::ids::{ObjectId, PlayerId};

use super::Payer;
use crate::events::{Event, ZoneCause};
use crate::library::top_cards;
use crate::state::GameState;
use crate::zones::Zone;

/// The cards the payer's library holds.
pub(super) fn most(payer: &Payer<'_>) -> u32 {
    let held = payer.state.zone(Zone::Library(payer.player)).len();
    u32::try_from(held).unwrap_or(u32::MAX)
}

/// Whether the library holds that many cards: a cost cannot mill what is not there.
pub(super) fn payable(payer: &Payer<'_>, amount: u32) -> bool {
    most(payer) >= amount
}

/// The top cards of the library, the top card first, each a move to the graveyard.
pub(super) fn events(state: &GameState, player: PlayerId, amount: u32) -> Vec<Event> {
    let wanted = usize::try_from(amount).unwrap_or(usize::MAX);
    top_cards(state, player, wanted)
        .into_iter()
        .map(|object| milled(player, object))
        .collect()
}

/// The events of a payment whose sources each mill from the same library: each source worked out its mill from the
/// library as it stands before the payment, so the mills are given the cards in turn, the top card to the first, and a
/// mill that finds the library empty mills nothing.
pub(crate) fn in_turn(state: &GameState, player: PlayerId, events: Vec<Event>) -> Vec<Event> {
    let from = Zone::Library(player);
    let is_mill = |event: &Event| matches!(event, Event::ZoneChanged { from: found, cause: ZoneCause::Milled, .. } if *found == from);
    let total = events.iter().filter(|event| is_mill(event)).count();
    if total < 2 {
        return events;
    }
    let mut cards = top_cards(state, player, total).into_iter();
    events
        .into_iter()
        .filter_map(|event| {
            if is_mill(&event) {
                cards.next().map(|object| milled(player, object))
            } else {
                Some(event)
            }
        })
        .collect()
}

/// One card of the library milled.
pub(crate) const fn milled(player: PlayerId, object: ObjectId) -> Event {
    Event::ZoneChanged {
        object,
        from: Zone::Library(player),
        to: Zone::Graveyard(player),
        cause: ZoneCause::Milled,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a mill cost is payable only when the library holds the cards, mills the top cards in order, and the mills of several sources of one payment take different cards, the top card to the first, with nothing to mill for a source that finds the library empty.
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};

    const ME: PlayerId = PlayerId(0);

    fn game() -> GameState {
        let setup = setup(
            vec![Vec::new(); 2],
            vec![vec![creature("Source", 1, 1)], Vec::new()],
        );
        GameState::from_setup(&setup).unwrap()
    }

    fn payer(state: &GameState) -> Payer<'_> {
        Payer {
            state,
            player: ME,
            source: state.zone(Zone::Battlefield)[0],
            barred: &[],
        }
    }

    #[rstest]
    #[case::nothing(0, true)]
    #[case::one(1, true)]
    #[case::the_whole_library(5, true)]
    #[case::more_than_there_is(6, false)]
    fn a_mill_is_payable_while_the_library_holds_the_cards(
        #[case] amount: u32,
        #[case] payable_now: bool,
    ) {
        let game = game();
        assert_eq!(payable(&payer(&game), amount), payable_now);
    }

    #[rstest]
    #[case::one(1, 1)]
    #[case::three(3, 3)]
    #[case::more_than_there_is(9, 5)]
    fn the_top_cards_are_milled_top_first(#[case] amount: u32, #[case] moved: usize) {
        let game = game();
        let top = top_cards(&game, ME, moved);
        assert_eq!(top.len(), moved);
        assert_eq!(
            events(&game, ME, amount),
            top.iter().map(|id| milled(ME, *id)).collect::<Vec<_>>()
        );
    }

    #[rstest]
    #[case::two_sources(2, 2)]
    #[case::three_sources(3, 3)]
    #[case::more_sources_than_cards(8, 5)]
    fn sources_of_one_payment_mill_different_cards_in_turn(
        #[case] sources: usize,
        #[case] moved: usize,
    ) {
        let game = game();
        let top = top_cards(&game, ME, 1)[0];
        let mut events: Vec<Event> = Vec::new();
        for _ in 0..sources {
            events.push(Event::LifePaid {
                player: ME,
                amount: 1,
            });
            events.push(milled(ME, top));
        }
        let fixed = in_turn(&game, ME, events);
        let objects: Vec<ObjectId> = fixed
            .iter()
            .filter_map(|event| match event {
                Event::ZoneChanged { object, .. } => Some(*object),
                _ => None,
            })
            .collect();
        assert_eq!(objects, top_cards(&game, ME, moved));
        let paid = fixed
            .iter()
            .filter(|event| matches!(event, Event::LifePaid { .. }))
            .count();
        assert_eq!(paid, sources, "the other events stay");
    }

    #[test]
    fn one_mill_is_left_as_it_is() {
        let game = game();
        let top = top_cards(&game, ME, 1)[0];
        let events = vec![milled(ME, top)];
        assert_eq!(in_turn(&game, ME, events.clone()), events);
    }
}
