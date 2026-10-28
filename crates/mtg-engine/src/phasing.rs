//! Phasing (CR 702.26): a permanent that phases out is treated as though it does not exist until it phases in during
//! its controller's untap step, as the same object with everything it had.

use std::collections::BTreeSet;

use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::controller_of;
use crate::error::EngineError;
use crate::events::Event;
use crate::fold::invalid;
use crate::object::{GameObject, ObjectRef, PermanentState, PhasedState, Phasing};
use crate::state::GameState;
use crate::zones::{Placement, Zone};

/// Whether `id` is a permanent that is phased out.
pub fn is_phased_out(state: &GameState, id: ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|object| object.zone() == Zone::PhasedOut)
}

/// The permanents in `zone` attached to `host`, as the object they were attached to, in id order.
fn attached_in(state: &GameState, zone: Zone, host: ObjectId) -> Vec<ObjectId> {
    let Some(host) = state.object(host).map(GameObject::reference) else {
        return Vec::new();
    };
    let mut found: Vec<ObjectId> = state
        .zone(zone)
        .iter()
        .copied()
        .filter(|id| {
            state
                .object(*id)
                .and_then(GameObject::permanent)
                .and_then(PermanentState::attached_to)
                .is_some_and(|attached| attached == host)
        })
        .collect();
    found.sort();
    found
}

/// The events that phase out the permanents `named` (CR 702.26): each one on the battlefield once, in order, and after
/// it the Auras, Equipment and Fortifications attached to it, which phase out with it (CR 702.26g). An object `named`
/// that is gone, not a permanent or phased out already is passed over.
pub fn out_events(state: &GameState, named: &[ObjectId]) -> Vec<Event> {
    w11b_out_events(state, named, W11bHold::default())
}

/// How long the permanents named by a phase-out stay out: until `until` leaves the battlefield (none: until the next
/// untap step of their controller), and whether they phase in tapped.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct W11bHold {
    /// The permanent whose leaving the battlefield phases them in.
    pub until: Option<ObjectRef>,
    /// They phase in tapped.
    pub tapped: bool,
}

/// `out_events` with a hold: the permanents named stay out until `hold.until` leaves the battlefield; what is attached
/// to them goes with them as usual and comes back with them.
pub fn w11b_out_events(state: &GameState, named: &[ObjectId], hold: W11bHold) -> Vec<Event> {
    let direct: BTreeSet<ObjectId> = named
        .iter()
        .copied()
        .filter(|id| {
            state
                .object(*id)
                .is_some_and(|o| o.zone() == Zone::Battlefield)
        })
        .collect();
    let mut done = BTreeSet::new();
    let mut events = Vec::new();
    let mut work: Vec<(ObjectId, bool)> = Vec::new();
    for id in named {
        if direct.contains(id) && done.insert(*id) {
            work.push((*id, false));
        }
        while let Some((id, indirect)) = work.pop() {
            let Some(controller) = controller_of(state, id) else {
                continue;
            };
            let hold = if indirect { W11bHold::default() } else { hold };
            events.push(Event::PermanentPhasedOut {
                object: id,
                controller,
                indirect,
                until: hold.until,
                tapped: hold.tapped,
            });
            for attached in attached_in(state, Zone::Battlefield, id).into_iter().rev() {
                // One named too phases out on its own and comes back on its own.
                if !direct.contains(&attached) && done.insert(attached) {
                    work.push((attached, true));
                }
            }
        }
    }
    events
}

/// The events that phase in what phases in as the untap step of `player` begins (CR 502.1, 702.26): the permanents that
/// were phased out as `player` controlled them, each followed by what phased out with it. An attachment whose host is no
/// longer phased out, because it left the game, phases in on its own.
pub fn in_events(state: &GameState, player: PlayerId) -> Vec<Event> {
    let phasing = |id: ObjectId| {
        state
            .object(id)
            .and_then(GameObject::permanent)
            .and_then(PermanentState::phasing)
    };
    let phased = state.zone(Zone::PhasedOut);
    let hosts_waiting = |attachment: ObjectId| {
        state
            .object(attachment)
            .and_then(GameObject::permanent)
            .and_then(PermanentState::attached_to)
            .is_some_and(|host| {
                state.object(host.id).is_some_and(|object| {
                    object.zone() == Zone::PhasedOut && object.counter() == host.counter
                })
            })
    };
    let mut events = Vec::new();
    for id in phased {
        // One held until a permanent leaves phases in then, not in an untap step.
        let Some(own) = phasing(*id).filter(|own| own.controller == player && own.until.is_none())
        else {
            continue;
        };
        if own.indirect && hosts_waiting(*id) {
            continue;
        }
        events.extend(w11b_with_attachments(state, *id));
    }
    events
}

/// The events that phase in `id` and, after it, what phased out with it.
fn w11b_with_attachments(state: &GameState, id: ObjectId) -> Vec<Event> {
    let indirect = |id: ObjectId| {
        state
            .object(id)
            .and_then(GameObject::permanent)
            .and_then(PermanentState::phasing)
            .is_some_and(|own| own.indirect)
    };
    let mut events = Vec::new();
    let mut work = vec![id];
    while let Some(id) = work.pop() {
        events.push(Event::PermanentPhasedIn { object: id });
        work.extend(
            attached_in(state, Zone::PhasedOut, id)
                .into_iter()
                .rev()
                .filter(|attached| indirect(*attached)),
        );
    }
    events
}

/// The events that phase in every permanent held out until a permanent that has left the battlefield, with what phased
/// out with it. Like a card exiled until a permanent leaves (CR 610.3a), this is no triggered ability: the control loop
/// runs it before anything else is checked. A permanent the source phased out is still the source, so it holds on.
pub(crate) fn w11b_releases(state: &GameState) -> Vec<Event> {
    state
        .zone(Zone::PhasedOut)
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .and_then(GameObject::permanent)
                .and_then(PermanentState::phasing)
                .and_then(|own| own.until)
                .is_some_and(|by| !crate::exile_return::holds(state, by))
        })
        .flat_map(|id| w11b_with_attachments(state, *id))
        .collect()
}

/// The events that phase out the permanents with phasing that `player` controls as their untap step begins (CR 502.1,
/// 702.26a), each with what is attached to it; it happens together with what phases in (`in_events`).
pub fn w7e_keyword_out_events(state: &GameState, player: PlayerId) -> Vec<Event> {
    let named: Vec<ObjectId> = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            controller_of(state, *id) == Some(player)
                && crate::keywords::has_keyword(state, *id, &mtg_cards::keyword::Keyword::Phasing)
        })
        .collect();
    out_events(state, &named)
}

/// The fold of `Event::PermanentPhasedOut`: the permanent phases out as the event says.
pub(crate) fn w11b_fold_out_event(state: &mut GameState, event: &Event) -> Result<(), EngineError> {
    let Event::PermanentPhasedOut {
        object,
        controller,
        indirect,
        until,
        tapped,
    } = event
    else {
        return Ok(());
    };
    let phasing = Phasing {
        controller: *controller,
        indirect: *indirect,
        until: *until,
        tapped: *tapped,
    };
    fold_out(state, *object, phasing)
}

/// A permanent phases out: it leaves the battlefield's list without leaving the battlefield, and leaves combat (CR 506.4).
pub(crate) fn fold_out(
    state: &mut GameState,
    id: ObjectId,
    phasing: Phasing,
) -> Result<(), EngineError> {
    let object = state
        .objects
        .get_mut(&id)
        .filter(|object| object.zone == Zone::Battlefield)
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent that is phased in")))?;
    let permanent = object
        .permanent
        .as_mut()
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent")))?;
    permanent.phased = PhasedState(Some(phasing));
    object.zone = Zone::PhasedOut;
    state.zones.remove(Zone::Battlefield, id);
    state.zones.insert(Zone::PhasedOut, id, Placement::Top);
    state.combat.remove_object(id);
    state.combat.planeswalker_left(id);
    // "For each creature that phased out this way" reads what the resolving spell or ability phased out.
    let reference = object.reference();
    if let Some(resolving) = state.resolving.as_mut() {
        resolving.w11b_phased.push(reference);
    }
    Ok(())
}

/// A permanent phases in: the same object, with the counters, damage and tapped state it had. It has been under its
/// controller's control all along, so a controller whose turn it is no longer holds it back for summoning sickness.
pub(crate) fn fold_in(state: &mut GameState, id: ObjectId) -> Result<(), EngineError> {
    let active = state.turn.active();
    let object = state
        .objects
        .get_mut(&id)
        .filter(|object| object.zone == Zone::PhasedOut)
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent that is phased out")))?;
    let permanent = object
        .permanent
        .as_mut()
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent")))?;
    let phasing = permanent.phased.0.take();
    if phasing.is_some_and(|phasing| phasing.tapped) {
        permanent.tapped = true;
    }
    if phasing.is_some_and(|phasing| phasing.controller == active) {
        permanent.new_to = permanent.sick_for(active).then_some(active);
        permanent.summoning_sick = false;
        permanent.observed = Some(active);
    }
    object.zone = Zone::Battlefield;
    state.zones.remove(Zone::PhasedOut, id);
    state.zones.insert(Zone::Battlefield, id, Placement::Top);
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Goal: a permanent phases out of every scan of the battlefield and in again as the same object with the state it had, at tables of two to four seats; only the untap step of the player it phased out for phases it in, it is controlled by that player while it is out, and what is not a permanent on the battlefield cannot phase out.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;

    /// Every seat controls a creature named after it; seat 0 also controls a Bear.
    fn table(seats: usize) -> GameState {
        let mut battlefields: Vec<Vec<_>> = (0..seats)
            .map(|seat| vec![creature(&format!("Seat {seat}"), 2, 2)])
            .collect();
        battlefields[0].push(creature("Bear", 1, 1));
        state(vec![Vec::new(); seats], battlefields)
    }

    fn id(game: &GameState, name: &str) -> ObjectId {
        game.objects()
            .find(|object| object.card().name == name)
            .map(GameObject::id)
            .unwrap()
    }

    fn apply(game: &mut GameState, events: &[Event]) {
        for event in events {
            apply_event(game, event).unwrap();
        }
    }

    #[rstest]
    #[case::two_seats_untapped(2, false)]
    #[case::two_seats_tapped(2, true)]
    #[case::three_seats_untapped(3, false)]
    #[case::three_seats_tapped(3, true)]
    #[case::four_seats_untapped(4, false)]
    #[case::four_seats_tapped(4, true)]
    fn a_permanent_phases_out_and_in_as_the_same_object(
        #[case] seats: usize,
        #[case] tapped: bool,
    ) {
        let mut game = table(seats);
        let bear = id(&game, "Bear");
        if tapped {
            apply(&mut game, &[Event::PermanentTapped { object: bear }]);
        }
        let (counter, hash) = (game.object(bear).unwrap().counter(), game.state_hash());
        let out = out_events(&game, &[bear]);
        apply(&mut game, &out);
        assert!(is_phased_out(&game, bear));
        assert!(!game.zone(Zone::Battlefield).contains(&bear));
        assert_eq!(game.zone(Zone::PhasedOut), [bear]);
        assert_ne!(game.state_hash(), hash);
        // It is not found by a scan, but it is still an object with the state it had.
        let held = game.object(bear).unwrap();
        assert_eq!(held.permanent().unwrap().tapped(), tapped);
        assert_eq!(held.counter(), counter);
        let back = in_events(&game, PlayerId(0));
        apply(&mut game, &back);
        assert!(!is_phased_out(&game, bear));
        assert_eq!(game.zone(Zone::PhasedOut), [] as [ObjectId; 0]);
        let held = game.object(bear).unwrap();
        assert_eq!(held.counter(), counter, "phasing is no new object");
        assert_eq!(held.permanent().unwrap().tapped(), tapped);
        assert_eq!(game.zone(Zone::Battlefield).len(), seats + 1);
    }

    #[rstest]
    #[case::two_seats(2)]
    #[case::three_seats(3)]
    #[case::four_seats(4)]
    fn w7e_only_the_permanents_with_phasing_of_the_untapping_player_phase_out(
        #[case] seats: usize,
    ) {
        let phaser = |name: &str| {
            std::sync::Arc::new(
                mtg_cards::CardDefinition::new(name)
                    .with_type(mtg_core::card::CardType::Creature)
                    .with_power_toughness(1, 1)
                    .with_rules(mtg_cards::ability::Ability::Keyword(
                        mtg_cards::keyword::Keyword::Phasing,
                    )),
            )
        };
        let mut battlefields: Vec<Vec<_>> = (0..seats)
            .map(|seat| vec![phaser(&format!("Phaser {seat}")), creature("Bear", 2, 2)])
            .collect();
        battlefields[0].push(creature("Other", 1, 1));
        let mut game = state(vec![Vec::new(); seats], battlefields);
        for seat in 0..seats {
            let out = w7e_keyword_out_events(&game, PlayerId(seat as u64));
            assert_eq!(out.len(), 1, "one phaser for seat {seat}");
            let Some(Event::PermanentPhasedOut { object, .. }) = out.first() else {
                panic!("{out:?}");
            };
            assert_eq!(*object, id(&game, &format!("Phaser {seat}")));
            apply(&mut game, &out);
            // Out, it is no longer found, so the next untap step only phases it in.
            assert_eq!(
                w7e_keyword_out_events(&game, PlayerId(seat as u64)).len(),
                0
            );
            assert_eq!(in_events(&game, PlayerId(seat as u64)).len(), 1);
        }
    }

    #[rstest]
    #[case::two_seats(2)]
    #[case::three_seats(3)]
    #[case::four_seats(4)]
    fn only_the_untap_step_of_the_player_it_phased_out_for_phases_a_permanent_in(
        #[case] seats: usize,
    ) {
        let mut game = table(seats);
        let everything: Vec<ObjectId> = game.zone(Zone::Battlefield).to_vec();
        let out = out_events(&game, &everything);
        apply(&mut game, &out);
        assert_eq!(game.zone(Zone::Battlefield), [] as [ObjectId; 0]);
        for seat in 0..seats {
            let coming: Vec<ObjectId> = in_events(&game, PlayerId::from_index(seat))
                .iter()
                .map(|event| match event {
                    Event::PermanentPhasedIn { object } => *object,
                    other => panic!("unexpected {other:?}"),
                })
                .collect();
            let want = if seat == 0 { 2 } else { 1 };
            assert_eq!(coming.len(), want, "seat {seat}");
            assert!(
                coming
                    .iter()
                    .all(|object| controller_of(&game, *object) == Some(PlayerId::from_index(seat)))
            );
        }
        assert_eq!(
            in_events(&game, PlayerId::from_index(seats)),
            Vec::<Event>::new()
        );
    }

    #[rstest]
    #[case::two_seats(2)]
    #[case::four_seats(4)]
    fn a_name_that_repeats_or_is_not_on_the_battlefield_phases_out_once_at_most(
        #[case] seats: usize,
    ) {
        let game = table(seats);
        let bear = id(&game, "Bear");
        let library = game.zone(Zone::Library(PlayerId(0)))[0];
        let events = out_events(&game, &[bear, library, bear]);
        assert_eq!(
            events,
            [Event::PermanentPhasedOut {
                object: bear,
                controller: PlayerId(0),
                indirect: false,
                until: None,
                tapped: false,
            }]
        );
        let mut game = game;
        let first = out_events(&game, &[bear])[0].clone();
        assert!(apply_event(&mut game, &first).is_ok());
        let again = Event::PermanentPhasedOut {
            object: bear,
            controller: PlayerId(0),
            indirect: false,
            until: None,
            tapped: false,
        };
        assert!(apply_event(&mut game, &again).is_err(), "it is out already");
        assert_eq!(out_events(&game, &[bear]), Vec::<Event>::new());
        assert!(apply_event(&mut game, &Event::PermanentPhasedIn { object: library }).is_err());
    }
}
