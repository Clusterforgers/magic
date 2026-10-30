//! Soulbond (CR 702.95): the pairing of two creatures of one controller. The pair is state of both permanents
//! (`Pairing`), made by `Event::Paired` and ended by `Event::Unpaired`, which the state-based pass logs when either creature
//! has left the battlefield, stopped being a creature or changed controller.

use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

use mtg_core::ids::ObjectId;

use crate::characteristics::{characteristics, controller_of_object};
use crate::error::EngineError;
use crate::events::Event;
use crate::fold::invalid;
use crate::object::{GameObject, ObjectRef};
use crate::state::GameState;
use crate::zones::Zone;

/// The creature a permanent is paired with, named as it was when paired; `GameState::partner_of` says whether that is still
/// so. An unpaired permanent adds nothing to the hash of the game.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pairing(pub(crate) Option<ObjectRef>);

impl Hash for Pairing {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if let Some(partner) = &self.0 {
            partner.hash(state);
        }
    }
}

impl GameState {
    /// The creature `id` is paired with, while both are still the very objects that were paired and on the battlefield
    /// (CR 702.95e, 400.7): a partner that left, even to come back, is no partner. The pair is mutual.
    pub fn partner_of(&self, id: ObjectId) -> Option<ObjectId> {
        let object = self.object(id).filter(|o| o.zone() == Zone::Battlefield)?;
        let held = object.permanent()?.paired_with()?;
        let partner = self
            .object(held.id)
            .filter(|p| p.zone() == Zone::Battlefield && p.reference() == held)?;
        (partner.permanent()?.paired_with() == Some(object.reference())).then_some(held.id)
    }
}

/// Pairs `object` with `with` (CR 702.95a): both are permanents on the battlefield, unpaired as far as `partner_of` reads.
pub(crate) fn fold_paired(
    state: &mut GameState,
    object: ObjectId,
    with: ObjectId,
) -> Result<(), EngineError> {
    if object == with {
        return Err(invalid("a creature cannot be paired with itself"));
    }
    let on_battlefield = |id| {
        state
            .object(id)
            .filter(|o| o.zone() == Zone::Battlefield && o.permanent().is_some())
            .map(GameObject::reference)
    };
    let (Some(mine), Some(theirs)) = (on_battlefield(object), on_battlefield(with)) else {
        return Err(invalid(format!(
            "{object:?} and {with:?} are not both on the battlefield to be paired"
        )));
    };
    if state.partner_of(object).is_some() || state.partner_of(with).is_some() {
        return Err(invalid(format!("{object:?} or {with:?} is paired already")));
    }
    for (id, partner) in [(object, theirs), (with, mine)] {
        if let Some(permanent) = state
            .objects
            .get_mut(&id)
            .and_then(|o| o.permanent.as_mut())
        {
            permanent.paired = Pairing(Some(partner));
        }
    }
    state.possible.pairs = true;
    Ok(())
}

/// Ends the pairing of `object` and `with` (CR 702.95e): each of the two that still holds the other as its partner lets go; one
/// that has left the battlefield holds nothing. The event is refused when neither held the other.
pub(crate) fn fold_unpaired(
    state: &mut GameState,
    object: ObjectId,
    with: ObjectId,
) -> Result<(), EngineError> {
    let mut released = false;
    for (id, other) in [(object, with), (with, object)] {
        if let Some(permanent) = state
            .objects
            .get_mut(&id)
            .and_then(|o| o.permanent.as_mut())
            .filter(|p| p.paired.0.is_some_and(|held| held.id == other))
        {
            permanent.paired = Pairing::default();
            released = true;
        }
    }
    if released {
        Ok(())
    } else {
        Err(invalid(format!("{object:?} is not paired with {with:?}")))
    }
}

/// Whether the pair `id` holds with `partner` has ended: the partner is no longer the object paired, either creature is no
/// longer a creature, or they are controlled by different players, or the controller has left the game.
fn broken(state: &GameState, id: ObjectId, partner: ObjectRef) -> bool {
    let (Some(mine), Some(theirs)) = (state.object(id), state.object(partner.id)) else {
        return true;
    };
    state.partner_of(id) != Some(partner.id)
        || !state.is_alive(mine.controller())
        || controller_of_object(state, mine) != controller_of_object(state, theirs)
        || ![id, partner.id]
            .into_iter()
            .all(|object| characteristics(state, object).is_some_and(|chars| chars.is_creature()))
}

/// The pairs that end in this pass of the state-based actions, one `Unpaired` each (CR 702.95e). A pair whose creatures both
/// leave in the same pass has nothing left to release, and a creature that leaves in it releases its partner in the next.
pub(crate) fn state_based(state: &GameState, leaving: &[ObjectId]) -> Vec<Event> {
    let mut events = Vec::new();
    if !state.possible.pairs {
        return events;
    }
    let mut ended = BTreeSet::new();
    for &id in state.zone(Zone::Battlefield) {
        let Some(partner) = state
            .object(id)
            .and_then(GameObject::permanent)
            .and_then(crate::object::PermanentState::paired_with)
        else {
            continue;
        };
        let key = (id.min(partner.id), id.max(partner.id));
        if (leaving.contains(&id) && leaving.contains(&partner.id))
            || !broken(state, id, partner)
            || !ended.insert(key)
        {
            continue;
        }
        events.push(Event::Unpaired {
            object: id,
            with: partner.id,
        });
    }
    events
}

#[cfg(test)]
mod tests {
    //! Goal: a pairing is state of both creatures that `Paired` makes and `Unpaired` ends; `partner_of` reads it only while both are the very objects paired, a creature is in one pair at most, the state-based pass ends a pair when a partner leaves, stops being a creature or changes controller, a game that was paired and is not any more hashes as one never paired, and a setup can start a pair that both creatures name.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::error::SetupError;
    use crate::events::ZoneCause;
    use crate::fixtures::{creature, setup, state};
    use crate::fold::apply_event;
    use crate::setup::PresetPermanent;

    fn rock() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new("Rock").with_type(CardType::Artifact))
    }

    /// Seat 0 has Ann (0), Bob (1), Cy (2) and Rock (3, an artifact); seat 1 has Dee (4).
    fn game() -> (GameState, Vec<ObjectId>) {
        let game = state(
            vec![vec![], vec![]],
            vec![
                vec![
                    creature("Ann", 1, 1),
                    creature("Bob", 2, 2),
                    creature("Cy", 3, 3),
                    rock(),
                ],
                vec![creature("Dee", 1, 1)],
            ],
        );
        let ids = game.zone(Zone::Battlefield).to_vec();
        (game, ids)
    }

    fn pair(game: &mut GameState, a: ObjectId, b: ObjectId) {
        apply_event(game, &Event::Paired { object: a, with: b }).unwrap();
    }

    fn leave(game: &mut GameState, id: ObjectId, to: Zone) {
        let event = Event::ZoneChanged {
            object: id,
            from: Zone::Battlefield,
            to,
            cause: ZoneCause::Moved,
        };
        apply_event(game, &event).unwrap();
    }

    #[test]
    fn a_pair_is_mutual_and_one_creature_is_in_one_pair() {
        let (mut game, ids) = game();
        pair(&mut game, ids[0], ids[1]);
        assert_eq!(game.partner_of(ids[0]), Some(ids[1]));
        assert_eq!(game.partner_of(ids[1]), Some(ids[0]));
        assert_eq!(game.partner_of(ids[2]), None);
        for (a, b) in [(0, 2), (2, 1), (1, 0), (0, 0)] {
            let event = Event::Paired {
                object: ids[a],
                with: ids[b],
            };
            assert!(apply_event(&mut game, &event).is_err(), "{a} with {b}");
        }
        assert_eq!(game.partner_of(ids[0]), Some(ids[1]));
    }

    #[rstest]
    #[case::graveyard(Zone::Graveyard(PlayerId(0)))]
    #[case::exile(Zone::Exile)]
    #[case::hand(Zone::Hand(PlayerId(0)))]
    fn a_partner_that_left_is_no_partner_and_the_other_is_released(#[case] to: Zone) {
        let (mut game, ids) = game();
        pair(&mut game, ids[0], ids[1]);
        leave(&mut game, ids[1], to);
        assert_eq!(game.partner_of(ids[0]), None);
        let ended = state_based(&game, &[]);
        assert_eq!(
            ended,
            vec![Event::Unpaired {
                object: ids[0],
                with: ids[1]
            }]
        );
        apply_event(&mut game, &ended[0]).unwrap();
        let held = game.object(ids[0]).unwrap().permanent().unwrap();
        assert_eq!(held.paired_with(), None);
        assert!(state_based(&game, &[]).is_empty());
    }

    #[test]
    fn a_partner_that_came_back_is_a_new_object_that_can_be_paired_again() {
        let (mut game, ids) = game();
        pair(&mut game, ids[0], ids[1]);
        leave(&mut game, ids[1], Zone::Exile);
        let back = Event::ZoneChanged {
            object: ids[1],
            from: Zone::Exile,
            to: Zone::Battlefield,
            cause: ZoneCause::Moved,
        };
        apply_event(&mut game, &back).unwrap();
        assert_eq!(game.partner_of(ids[0]), None);
        assert_eq!(game.partner_of(ids[1]), None);
        pair(&mut game, ids[0], ids[1]);
        assert_eq!(game.partner_of(ids[0]), Some(ids[1]));
        assert_eq!(game.partner_of(ids[1]), Some(ids[0]));
        assert!(state_based(&game, &[]).is_empty());
    }

    #[rstest]
    #[case::both_creatures_of_one_controller(1, false)]
    #[case::a_partner_that_is_not_a_creature(3, true)]
    #[case::a_partner_of_another_controller(4, true)]
    fn the_state_based_pass_ends_a_pair_that_cannot_stand(#[case] with: usize, #[case] ends: bool) {
        let (mut game, ids) = game();
        pair(&mut game, ids[0], ids[with]);
        let ended = state_based(&game, &[]);
        // One event for the pair, whichever creature the pass meets first.
        assert_eq!(ended.len(), usize::from(ends));
        for event in &ended {
            apply_event(&mut game, event).unwrap();
        }
        assert_eq!(game.partner_of(ids[0]).is_some(), !ends);
    }

    #[test]
    fn a_pair_whose_creatures_both_leave_in_the_pass_has_nothing_to_release() {
        let (mut game, ids) = game();
        pair(&mut game, ids[0], ids[3]);
        assert!(state_based(&game, &[ids[0], ids[3]]).is_empty());
        assert_eq!(state_based(&game, &[ids[0]]).len(), 1);
    }

    #[test]
    fn an_unpair_needs_a_pair_and_a_game_that_ended_one_hashes_as_before() {
        let (mut game, ids) = game();
        let before = game.state_hash();
        let unpair = Event::Unpaired {
            object: ids[0],
            with: ids[1],
        };
        assert!(apply_event(&mut game, &unpair).is_err());
        pair(&mut game, ids[0], ids[1]);
        assert_ne!(game.state_hash(), before);
        apply_event(&mut game, &unpair).unwrap();
        assert_eq!(game.state_hash(), before);
    }

    fn naming(partner: Option<(usize, usize)>) -> PresetPermanent {
        PresetPermanent {
            paired_with: partner,
            ..PresetPermanent::new(creature("Twin", 1, 1))
        }
    }

    #[test]
    fn a_setup_starts_a_pair_across_seats_that_both_name() {
        let mut start = setup(vec![vec![], vec![]], vec![vec![], vec![]]);
        start.players[0].battlefield = vec![naming(Some((1, 0))), naming(None)];
        start.players[1].battlefield = vec![naming(Some((0, 0)))];
        let game = GameState::from_setup(&start).unwrap();
        let ids = game.zone(Zone::Battlefield).to_vec();
        assert_eq!(game.partner_of(ids[0]), Some(ids[2]));
        assert_eq!(game.partner_of(ids[2]), Some(ids[0]));
        assert_eq!(game.partner_of(ids[1]), None);
        // The pair is across controllers, so the first pass of the state-based actions ends it.
        assert_eq!(state_based(&game, &[]).len(), 1);
    }

    #[rstest]
    #[case::to_itself((0, 0))]
    #[case::to_a_seat_that_is_not_there((7, 0))]
    #[case::to_a_place_that_is_not_there((0, 5))]
    #[case::to_a_creature_that_names_no_one((0, 1))]
    fn a_pairing_nobody_names_back_is_refused_by_name(#[case] partner: (usize, usize)) {
        let mut start = setup(vec![vec![], vec![]], vec![vec![], vec![]]);
        start.players[0].battlefield = vec![naming(Some(partner)), naming(None)];
        assert_eq!(
            GameState::from_setup(&start).unwrap_err(),
            SetupError::BadPairing { seat: 0, index: 0 }
        );
    }
}
