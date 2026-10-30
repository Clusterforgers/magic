//! What a seat may identify beyond what the zone of a card shows: the cards a resolution showed to it, and the cards it
//! is asked to choose among or to put in an order (`docs/protocol.md`, Visibility).
//!
//! Both are facts about the state alone. What a resolution showed is a fold of its `LibraryLooked`, `CardsFound`,
//! `Scried` and `Revealed` events and ends with the resolution; what a seat is asked is what the step that waits
//! puts to it, which only that step can say.

use mtg_core::ids::{ObjectId, PlayerId};

use crate::decision::{Decision, DecisionKind};
use crate::object::{FaceDown, GameObject, ObjectRef};
use crate::resolve::{Progress, Shown, step};
use crate::state::{GameState, Stage};
use crate::zones::Zone;

impl GameState {
    /// Whether `viewer` may look at what the object is: anyone may unless it lies face down (CR 708), and then only
    /// the player its kind of face-down names (CR 702.143a: the owner of a foretold card; CR 708.3: the controller of a face-down spell or permanent). This is the one rule the
    /// views, the log and the snapshots read for a face-down object.
    pub fn may_look_at(&self, viewer: PlayerId, id: ObjectId) -> bool {
        let Some(object) = self.object(id) else {
            return false;
        };
        object.face_down().is_none_or(|how| {
            self.may_look_as(
                how,
                object.owner(),
                crate::characteristics::controller_of_object(self, object),
                viewer,
            )
        })
    }

    /// Whether `viewer` may look at a card of `owner` that lies face down as `how` under the control of `controller`. A
    /// hidden card (hideaway, CR 702.75a) is also looked at by the controller of the permanent that hid it while that
    /// permanent is still the one that did; the players who controlled it before are not tracked.
    pub fn may_look_as(
        &self,
        how: FaceDown,
        owner: PlayerId,
        controller: PlayerId,
        viewer: PlayerId,
    ) -> bool {
        if how.may_look(owner, controller, viewer) {
            return true;
        }
        let FaceDown::Hideaway { by } = how else {
            return false;
        };
        self.object(by.id).is_some_and(|hider| {
            hider.counter() == by.counter
                && hider.zone() == Zone::Battlefield
                && crate::characteristics::controller_of_object(self, hider) == viewer
        })
    }

    /// Whether `viewer` may identify the object beyond what its zone shows: the top card of a library that a rule
    /// reveals is shown to every seat and the top card of one its owner may look at to that owner, and the resolution in progress may have shown it to them, which holds while it is
    /// still the card it was shown: a new zone makes a new object (CR 400.7), and that one is judged by its zone alone.
    pub fn is_shown_to(&self, viewer: PlayerId, id: ObjectId) -> bool {
        let Some(object) = self.object(id) else {
            return false;
        };
        if let Zone::Library(owner) = object.zone() {
            if self.revealed_top(owner) == Some(id) {
                return true;
            }
            if self.looked_by_hand.contains(&(viewer, object.reference())) {
                return true;
            }
            // The owner may look at their own top card any time (a card rule, never the house rule).
            if viewer == owner
                && self.zone(Zone::Library(owner)).last() == Some(&id)
                && crate::rules::looks_at_top(self, owner)
            {
                return true;
            }
        }
        self.resolving().is_some_and(|resolving| {
            resolving.shown().iter().any(|shown| {
                shown.card == object.reference() && shown.to.is_none_or(|seat| seat == viewer)
            })
        })
    }
}

/// The top cards of `seat`'s library that it may identify, top first, up to the first it may not.
pub fn known_top(state: &GameState, seat: PlayerId) -> Vec<ObjectId> {
    state
        .zone(Zone::Library(seat))
        .iter()
        .rev()
        .take_while(|id| state.is_shown_to(seat, **id))
        .copied()
        .collect()
}

/// Records that `seat` looked at the top `count` cards of its library by hand, and forgets what it looked at before
/// that is no longer the object it was or no longer in the library.
pub(crate) fn look_by_hand(state: &mut GameState, seat: PlayerId, count: u32) {
    let library = Zone::Library(seat);
    let still = |state: &GameState, card: &ObjectRef| {
        state
            .object(card.id)
            .is_some_and(|object| object.reference() == *card && object.zone() == library)
    };
    let kept: Vec<_> = state
        .looked_by_hand
        .iter()
        .filter(|(who, card)| *who != seat || still(state, card))
        .copied()
        .collect();
    let looked: Vec<_> =
        crate::library::top_cards(state, seat, usize::try_from(count).unwrap_or(0))
            .into_iter()
            .filter_map(|id| state.object(id).map(GameObject::reference))
            .filter(|card| !kept.contains(&(seat, *card)))
            .map(|card| (seat, card))
            .collect();
    state.looked_by_hand = kept.into_iter().chain(looked).collect();
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
    if state.stage() == Stage::Mulligan {
        // The seat that scries after keeping its hand is asked about the top card of its library.
        return match crate::mulligan::scry_decision(state)? {
            Decision {
                player,
                kind: DecisionKind::ChooseCards(options),
            } => Some((player, options.from)),
            _ => None,
        };
    }
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
    //! Goal: what a resolution shows is known to exactly the seats it was shown to: the cards looked at, found or scried to that seat alone, the cards revealed to every seat, nothing outside a resolution, nothing once the resolution has ended or the card has become another object by changing zones; a foretold card is looked at by its owner alone until it changes zones.
    use rstest::rstest;

    use super::*;
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::library::{CardsFound, LibraryLooked, Revealed, Scried};
    use crate::object::FaceDown;
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

    #[rstest]
    #[case::the_owner(0, true)]
    #[case::an_opponent(1, false)]
    #[case::the_other_opponent(2, false)]
    #[case::a_spectator(3, false)]
    fn only_the_owner_may_look_at_a_foretold_card(#[case] viewer: usize, #[case] looks: bool) {
        let mut game = state(
            vec![vec![creature("Card", 1, 1)], vec![], vec![]],
            vec![vec![], vec![], vec![]],
        );
        let viewer = PlayerId::from_index(viewer);
        let card = game
            .objects()
            .find(|o| o.card().name == "Card")
            .map(GameObject::id)
            .unwrap();
        let from = game.object(card).unwrap().zone();
        assert!(game.may_look_at(viewer, card), "a face-up card is open");
        let foretell = Event::ZoneChanged {
            object: card,
            from,
            to: Zone::Exile,
            cause: ZoneCause::Foretold,
        };
        apply_event(&mut game, &foretell).unwrap();
        let object = game.object(card).unwrap();
        assert_eq!(
            object.face_down(),
            Some(FaceDown::Foretold {
                turn: game.turn().number()
            })
        );
        assert_eq!(game.may_look_at(viewer, card), looks);
        // A new zone makes a new object that lies face up (CR 400.7).
        let leave = Event::ZoneChanged {
            object: card,
            from: Zone::Exile,
            to: Zone::Graveyard(ME),
            cause: ZoneCause::Moved,
        };
        apply_event(&mut game, &leave).unwrap();
        assert_eq!(game.object(card).unwrap().face_down(), None);
        assert!(game.may_look_at(viewer, card));
    }

    /// Seat 0 owns the hidden card; the Hider on the battlefield of seat 0 hid it. `controller` then controls the Hider.
    #[rstest]
    #[case::the_owner_who_controls_it(0, 0, true)]
    #[case::the_owner_after_it_changed_hands(0, 1, true)]
    #[case::the_new_controller(1, 1, true)]
    #[case::a_seat_that_never_controlled_it(2, 1, false)]
    #[case::the_old_controller_is_not_tracked_unless_it_owns_the_card(2, 0, false)]
    fn a_hidden_card_is_looked_at_by_its_owner_and_the_controller_of_the_permanent(
        #[case] viewer: usize,
        #[case] controller: usize,
        #[case] looks: bool,
    ) {
        let mut game = state(
            vec![vec![creature("Card", 1, 1)], vec![], vec![]],
            vec![vec![creature("Hider", 1, 1)], vec![], vec![]],
        );
        let find = |game: &GameState, name: &str| {
            game.objects()
                .find(|o| o.card().name == name)
                .map(GameObject::id)
                .unwrap()
        };
        let (card, hider) = (find(&game, "Card"), find(&game, "Hider"));
        let by = game.object(hider).unwrap().reference();
        let from = game.object(card).unwrap().zone();
        let hide = Event::ZoneChanged {
            object: card,
            from,
            to: Zone::Exile,
            cause: ZoneCause::Hideaway { by },
        };
        apply_event(&mut game, &hide).unwrap();
        game.objects.get_mut(&hider).unwrap().controller = PlayerId::from_index(controller);
        let viewer = PlayerId::from_index(viewer);
        assert_eq!(
            game.object(card).unwrap().face_down(),
            Some(FaceDown::Hideaway { by })
        );
        assert_eq!(game.may_look_at(viewer, card), looks);
        // The permanent leaving ends the link: the card stays hidden and only its owner looks at it.
        let leave = Event::ZoneChanged {
            object: hider,
            from: Zone::Battlefield,
            to: Zone::Graveyard(ME),
            cause: ZoneCause::Destroyed,
        };
        apply_event(&mut game, &leave).unwrap();
        assert_eq!(game.may_look_at(viewer, card), viewer == ME);
    }

    #[test]
    fn nobody_may_look_at_an_object_the_game_does_not_have() {
        let game = game();
        assert!(!game.may_look_at(ME, ObjectId(9_999)));
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
