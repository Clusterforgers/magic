//! What a seat may identify beyond what the zone of a card shows: the cards a resolution showed to it, and the cards it
//! is asked to choose among or to put in an order (`docs/protocol.md`, Visibility).
//!
//! Both are facts about the state alone. What a resolution showed is a fold of its `LibraryLooked`, `CardsFound`,
//! `Scried` and `Revealed` events and ends with the resolution; what a seat is asked is what the step that waits
//! puts to it, which only that step can say.

use mtg_core::ids::{ObjectId, PlayerId};

use crate::decision::{Decision, DecisionKind};
use crate::object::GameObject;
use crate::resolve::{Progress, Shown, step};
use crate::state::GameState;

impl GameState {
    /// Whether the resolution in progress has shown `viewer` the object, which is still the card it was shown: a new
    /// zone makes a new object (CR 400.7), and that one is judged by its zone alone.
    pub fn is_shown_to(&self, viewer: PlayerId, id: ObjectId) -> bool {
        let (Some(resolving), Some(object)) = (self.resolving(), self.object(id)) else {
            return false;
        };
        resolving.shown().iter().any(|shown| {
            shown.card == object.reference() && shown.to.is_none_or(|seat| seat == viewer)
        })
    }
}

/// Records that the resolution in progress showed `cards` to `to`, or to every seat when it is `None`. Nothing is
/// recorded outside a resolution.
pub(crate) fn show(state: &mut GameState, to: Option<PlayerId>, cards: &[ObjectId]) {
    let shown: Vec<Shown> = cards
        .iter()
        .filter_map(|id| state.object(*id).map(GameObject::reference))
        .map(|card| Shown { to, card })
        .collect();
    if let Some(resolving) = state.resolving.as_mut() {
        for each in shown {
            if !resolving.shown.contains(&each) {
                resolving.shown.push(each);
            }
        }
    }
}

/// The seat the resolution in progress waits on and the cards of its question when it asks to choose cards or to put
/// cards in an order: that seat may identify them wherever they are. It is read from the step itself, which asks
/// the same question until it is answered, so it holds for a state rebuilt from the log as for the live one.
pub fn offered_cards(state: &GameState) -> Option<(PlayerId, Vec<ObjectId>)> {
    state.resolving()?;
    match step(state) {
        Ok(Progress::Ask(Decision { player, kind })) => match kind {
            DecisionKind::ChooseCards(options) => Some((player, options.from)),
            DecisionKind::OrderCards(options) => Some((player, options.cards)),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: what a resolution shows is known to exactly the seats it was shown to: the cards looked at, found or scried to that seat alone, the cards revealed to every seat, nothing outside a resolution, nothing once the resolution has ended or the card has become another object by changing zones.
    use rstest::rstest;

    use super::*;
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::library::{CardsFound, LibraryLooked, Revealed, Scried};
    use crate::resolve::Resolving;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    /// Three seats; a resolution of the Source on seat 0's battlefield is under way.
    fn game() -> GameState {
        let mut game = state(
            vec![vec![], vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![], vec![]],
        );
        let source = game.zone(Zone::Battlefield)[0];
        game.resolving = Some(Resolving::begin(source));
        game
    }

    fn top(game: &GameState, seat: usize, n: usize) -> Vec<ObjectId> {
        let mut cards = game
            .zone(Zone::Library(PlayerId::from_index(seat)))
            .to_vec();
        cards.reverse();
        cards.truncate(n);
        cards
    }

    fn who_sees(game: &GameState, card: ObjectId) -> Vec<usize> {
        (0..3)
            .filter(|seat| game.is_shown_to(PlayerId::from_index(*seat), card))
            .collect()
    }

    #[rstest]
    #[case::looked_at(0)]
    #[case::found(1)]
    #[case::scried(2)]
    #[case::revealed(3)]
    fn a_card_is_known_to_those_it_was_shown_to(#[case] how: usize) {
        let mut game = game();
        let cards = top(&game, 1, 2);
        // A scry puts back the top of its own library; the others look at or find in the library of seat 1.
        let seat = if how == 2 { PlayerId(1) } else { PlayerId(2) };
        let event = match how {
            0 => Event::LibraryLooked(LibraryLooked {
                player: seat,
                cards: cards.clone(),
            }),
            1 => Event::CardsFound(CardsFound {
                player: seat,
                zone: Zone::Library(PlayerId(1)),
                cards: cards.clone(),
            }),
            2 => Event::Scried(Scried {
                player: seat,
                top: vec![cards[0]],
                bottom: vec![cards[1]],
            }),
            _ => Event::Revealed(Revealed {
                player: seat,
                cards: cards.clone(),
            }),
        };
        apply_event(&mut game, &event).unwrap();
        let expected = if how == 3 {
            vec![0, 1, 2]
        } else {
            vec![seat.index()]
        };
        for card in &cards {
            assert_eq!(who_sees(&game, *card), expected);
        }
        let unseen = top(&game, 0, 1)[0];
        assert_eq!(who_sees(&game, unseen), Vec::<usize>::new());
    }

    #[test]
    fn a_card_that_changed_zones_is_another_object_and_not_known() {
        let mut game = game();
        let card = top(&game, 1, 1)[0];
        let event = Event::Revealed(Revealed {
            player: ME,
            cards: vec![card],
        });
        apply_event(&mut game, &event).unwrap();
        assert_eq!(who_sees(&game, card), [0, 1, 2]);
        let moved = Event::ZoneChanged {
            object: card,
            from: Zone::Library(PlayerId(1)),
            to: Zone::Hand(PlayerId(1)),
            cause: ZoneCause::Moved,
        };
        apply_event(&mut game, &moved).unwrap();
        assert_eq!(who_sees(&game, card), Vec::<usize>::new());
    }

    #[test]
    fn nothing_is_shown_outside_a_resolution_and_nothing_outlasts_one() {
        let mut game = game();
        let card = top(&game, 1, 1)[0];
        let reveal = Event::Revealed(Revealed {
            player: ME,
            cards: vec![card],
        });
        apply_event(&mut game, &reveal).unwrap();
        game.resolving = None;
        assert_eq!(who_sees(&game, card), Vec::<usize>::new());
        apply_event(&mut game, &reveal).unwrap();
        assert_eq!(
            who_sees(&game, card),
            Vec::<usize>::new(),
            "a reveal outside a resolution shows nothing"
        );
    }

    #[test]
    fn the_same_card_shown_twice_is_listed_once() {
        let mut game = game();
        let card = top(&game, 1, 1)[0];
        let reveal = Event::Revealed(Revealed {
            player: ME,
            cards: vec![card],
        });
        apply_event(&mut game, &reveal).unwrap();
        apply_event(&mut game, &reveal).unwrap();
        assert_eq!(game.resolving().unwrap().shown().len(), 1);
    }

    #[test]
    fn nothing_is_offered_when_nothing_resolves_or_the_step_asks_nothing() {
        let mut game = game();
        assert_eq!(
            offered_cards(&game),
            None,
            "a Source with no rules resolves without a question"
        );
        game.resolving = None;
        assert_eq!(offered_cards(&game), None);
    }
}
