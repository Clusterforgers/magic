//! Counters for a permanent that leaves in the same batch (CR 400.7): an effect run in place of one event of the batch
//! reads the state before the batch, so it may name a permanent that another event of the batch moves first ("If a
//! creature an opponent controls would die, exile it instead. When you do, put +1/+1 counters on this creature." while
//! this creature dies too, by a sacrifice or by lethal combat damage). That permanent is a new object afterwards and
//! gets nothing.

use std::collections::BTreeSet;

use mtg_core::ids::ObjectId;

use crate::choose::Target;
use crate::events::Event;
use crate::state::GameState;

/// Drops the counters put on or removed from an object as it was before the batch (its incarnation in `state`) after an
/// earlier event of the batch moved it to another zone; counters for the object it becomes (a permanent that enters
/// with counters) stay.
pub(super) fn w13a_without_counters_on_moved(state: &GameState, events: &mut Vec<Event>) {
    let mut moved = BTreeSet::new();
    events.retain(|event| {
        let Event::CountersChanged(changed) = event else {
            moved.extend(w13a_moved_object(event));
            return true;
        };
        let Target::Object(named) = changed.on else {
            return true;
        };
        let before = state
            .object(named.id)
            .map(crate::object::GameObject::counter);
        !(moved.contains(&named.id) && before.is_some_and(|counter| named.counter <= counter))
    });
}

/// The object an event moves to another zone or out of the game: a zone change, a death by state-based actions (lethal
/// damage in combat), a card drawn or discarded and an object whose owner left the game.
fn w13a_moved_object(event: &Event) -> Option<ObjectId> {
    match event {
        Event::ZoneChanged { object, .. }
        | Event::CreatureDied { object, .. }
        | Event::ObjectLeftGame { object }
        | Event::CardDrawn { object, .. }
        | Event::CardDiscarded { object, .. } => Some(*object),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a death replaced by an exile with counters for the source keeps the batch valid when the source dies in
    //! the same batch (Ravenous Slime and a creature of an opponent sacrificed together or dead of lethal combat damage),
    //! counters for the object a move of the batch makes stay, and the counters still come when the source stays.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_cards::value::CounterKind;
    use mtg_core::card::CardType;
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::super::{Processed, process};

    use crate::choose::Target;
    use crate::counters::CountersChanged;
    use crate::events::{DeathCause, Event, ZoneCause};
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::object::ObjectRef;
    use crate::state::GameState;
    use crate::zones::Zone;

    const SLIME: &str = r#"{"Replacement":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"Opponents"}],"from":"Battlefield","to":"Graveyard"}},"by":{"Instead":[{"Exile":{"what":"Triggering"}},{"PutCounters":{"on":"This","counter":"+1/+1","count":{"Stat":{"of":"Triggering","stat":"Power"}}}}]}}}"#;

    /// Seat 0 controls the Slime; seat 1 controls a Bear.
    fn game() -> GameState {
        let rule: Ability = serde_json::from_str(SLIME).unwrap();
        let slime = CardDefinition::new("Slime")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rule);
        state(
            vec![vec![], vec![]],
            vec![vec![Arc::new(slime)], vec![creature("Bear", 2, 2)]],
        )
    }

    /// The death of the creature: by state-based actions for lethal damage, or a sacrifice.
    fn died(object: ObjectId, owner: PlayerId, lethal: bool) -> Event {
        if lethal {
            return Event::CreatureDied {
                object,
                cause: DeathCause::LethalDamage,
            };
        }
        Event::ZoneChanged {
            object,
            from: Zone::Battlefield,
            to: Zone::Graveyard(owner),
            cause: ZoneCause::Sacrificed,
        }
    }

    #[rstest]
    #[case::slime_sacrificed_too(true, false)]
    #[case::slime_stays(false, false)]
    #[case::slime_dies_in_combat_too(true, true)]
    #[case::slime_stays_in_combat(false, true)]
    fn the_batch_applies_and_the_counters_come_only_while_the_slime_stays(
        #[case] together: bool,
        #[case] lethal: bool,
    ) {
        let mut game = game();
        let slime = game.zone(Zone::Battlefield)[0];
        let bear = game.zone(Zone::Battlefield)[1];
        let mut batch = vec![died(bear, PlayerId(1), lethal)];
        if together {
            batch.insert(0, died(slime, PlayerId(0), lethal));
        }
        let Processed::Events(events) = process(&game, batch, &[]).unwrap() else {
            panic!("nothing is asked");
        };
        let counters = events
            .iter()
            .filter(|event| matches!(event, Event::CountersChanged(_)))
            .count();
        assert_eq!(counters, usize::from(!together), "{events:?}");
        for event in &events {
            apply_event(&mut game, event).unwrap_or_else(|error| panic!("{event:?}: {error}"));
        }
        assert_eq!(
            game.object(bear).map(crate::object::GameObject::zone),
            Some(Zone::Exile)
        );
    }

    #[test]
    fn counters_for_the_object_a_move_of_the_batch_makes_stay() {
        let mut game = game();
        let slime = game.zone(Zone::Battlefield)[0];
        let before = game
            .object(slime)
            .map_or(0, crate::object::GameObject::counter);
        let put = |counter| {
            Event::CountersChanged(CountersChanged {
                on: Target::Object(ObjectRef { id: slime, counter }),
                counter: CounterKind::PlusOnePlusOne,
                delta: 1,
            })
        };
        let exiled = Event::ZoneChanged {
            object: slime,
            from: Zone::Battlefield,
            to: Zone::Exile,
            cause: ZoneCause::Exiled,
        };
        let Processed::Events(events) =
            process(&game, vec![exiled.clone(), put(before + 1)], &[]).unwrap()
        else {
            panic!("nothing is asked");
        };
        assert_eq!(events, [exiled, put(before + 1)]);
        for event in &events {
            apply_event(&mut game, event).unwrap_or_else(|error| panic!("{event:?}: {error}"));
        }
    }
}
