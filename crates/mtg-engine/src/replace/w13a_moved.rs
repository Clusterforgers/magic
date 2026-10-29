//! Counters for a permanent that leaves in the same batch (CR 400.7): an effect run in place of one event of the batch
//! reads the state before the batch, so it may name a permanent that another event of the batch moves first ("If a
//! creature an opponent controls would die, exile it instead. When you do, put +1/+1 counters on this creature." while
//! this creature dies too). That permanent is a new object afterwards and gets nothing.

use std::collections::BTreeSet;

use crate::choose::Target;
use crate::events::Event;

/// Drops the counters put on or removed from an object after an earlier event of the batch moved it to another zone.
pub(super) fn w13a_without_counters_on_moved(events: &mut Vec<Event>) {
    let mut moved = BTreeSet::new();
    events.retain(|event| match event {
        Event::ZoneChanged { object, .. } => {
            moved.insert(*object);
            true
        }
        Event::CountersChanged(changed) => match changed.on {
            Target::Object(named) => !moved.contains(&named.id),
            Target::Player(_) => true,
        },
        _ => true,
    });
}

#[cfg(test)]
mod tests {
    //! Goal: a death replaced by an exile with counters for the source keeps the batch valid when the source dies in
    //! the same batch (Ravenous Slime and a creature of an opponent sacrificed together), and the counters still come
    //! when the source stays.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_core::card::CardType;
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::super::{Processed, process};
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
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

    fn sacrificed(object: ObjectId, owner: PlayerId) -> Event {
        Event::ZoneChanged {
            object,
            from: Zone::Battlefield,
            to: Zone::Graveyard(owner),
            cause: ZoneCause::Sacrificed,
        }
    }

    #[rstest]
    #[case::slime_dies_too(true)]
    #[case::slime_stays(false)]
    fn the_batch_applies_and_the_counters_come_only_while_the_slime_stays(#[case] together: bool) {
        let mut game = game();
        let slime = game.zone(Zone::Battlefield)[0];
        let bear = game.zone(Zone::Battlefield)[1];
        let mut batch = vec![sacrificed(bear, PlayerId(1))];
        if together {
            batch.insert(0, sacrificed(slime, PlayerId(0)));
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
}
