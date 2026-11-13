//! `ReplaceBy::PreventByCounters` (Rock Hydra): for each 1 damage that would be dealt to the permanent, while it has a
//! counter of the kind, one counter is removed and that 1 damage is prevented. Damage that can't be prevented still
//! removes the counters (the Rock Hydra ruling).

use mtg_cards::trigger::ReplaceBy;
use mtg_cards::value::CounterKind;

use super::{Candidate, Item, Origin, Pass, Slot};
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;

impl Pass<'_> {
    /// Removes as many counters of the kind as the damage at `at` and the permanent's counters allow, and prevents that
    /// much of the damage unless it can't be prevented; counters already queued for removal in the batch are gone.
    pub(super) fn r5507_prevent_by_counters(
        &mut self,
        at: usize,
        candidate: &Candidate,
        kind: &CounterKind,
        applied: Vec<Origin>,
    ) {
        let (amount, unpreventable) = match &self.slots[at].item {
            Item::One(Event::DamageDealt {
                amount,
                unpreventable,
                ..
            }) => (*amount, *unpreventable),
            _ => (0, false),
        };
        let held = self
            .state
            .object(candidate.object.id)
            .filter(|card| card.counter() == candidate.object.counter)
            .and_then(|card| card.permanent().map(|held| held.counters_of(kind)))
            .unwrap_or(0);
        let queued: u32 = self
            .slots
            .iter()
            .filter_map(|slot| match &slot.item {
                Item::One(Event::CountersChanged(CountersChanged { on, counter, delta }))
                    if *on == Target::Object(candidate.object) && counter == kind && *delta < 0 =>
                {
                    Some(delta.unsigned_abs())
                }
                _ => None,
            })
            .sum();
        let removed = amount.min(held.saturating_sub(queued));
        if removed == 0 {
            self.slots[at].applied = applied;
            return;
        }
        let unit = self.unit(&self.slots[at].item);
        if unpreventable || self.unpreventable(&self.slots[at], &unit) {
            self.slots[at].applied.clone_from(&applied);
        } else {
            self.prevent(
                at,
                candidate,
                &ReplaceBy::PreventUpTo(removed),
                applied.clone(),
            );
        }
        self.slots.insert(
            at,
            Slot {
                item: Item::One(Event::CountersChanged(CountersChanged {
                    on: Target::Object(candidate.object),
                    counter: kind.clone(),
                    delta: -i32::try_from(removed).unwrap_or(i32::MAX),
                })),
                applied,
                frozen: false,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a prevention by counters prevents as much of the damage as the permanent has counters of the kind and
    //! removes that many, counting the removals already queued in the batch, prevents nothing without counters, still
    //! removes the counters for damage that can't be prevented, and covers only damage to its own permanent; at two,
    //! three and four seats.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_cards::value::CounterKind;
    use mtg_core::card::CardType;
    use mtg_core::ids::ObjectId;
    use rstest::rstest;

    use super::super::{Processed, process};
    use crate::choose::Target;
    use crate::combat::{DamageForm, DamageTarget};
    use crate::counters::CountersChanged;
    use crate::events::Event;
    use crate::fixtures::{creature, state};
    use crate::state::GameState;
    use crate::zones::Zone;

    const HYDRA: &str =
        r#"{"Replacement":{"when":{"Damage":{"to":"This"}},"by":{"PreventByCounters":"+1/+1"}}}"#;
    const UNPREVENTABLE: &str = r#"{"Rule":{"rule":{"DamageCantBePrevented":{}}}}"#;

    /// Seat 0 controls a 0/0 Hydra with `rules` and `counters` +1/+1 counters; seat 1 controls a Bear.
    fn game(seats: usize, rules: &[&str], counters: i32) -> GameState {
        let mut hydra = CardDefinition::new("Hydra")
            .with_type(CardType::Creature)
            .with_power_toughness(0, 0);
        for rule in rules {
            hydra = hydra.with_rules(serde_json::from_str::<Ability>(rule).unwrap());
        }
        let mut battlefields = vec![vec![Arc::new(hydra)], vec![creature("Bear", 2, 2)]];
        battlefields.resize(seats, Vec::new());
        let mut game = state(vec![vec![]; seats], battlefields);
        if counters > 0 {
            let on = game.object(at(&game, 0)).unwrap().reference();
            crate::fold::apply_event(
                &mut game,
                &Event::CountersChanged(CountersChanged {
                    on: Target::Object(on),
                    counter: CounterKind::PlusOnePlusOne,
                    delta: counters,
                }),
            )
            .unwrap();
        }
        game
    }

    fn at(game: &GameState, index: usize) -> ObjectId {
        game.zone(Zone::Battlefield)[index]
    }

    fn hit(source: ObjectId, to: ObjectId, amount: u32) -> Event {
        Event::DamageDealt {
            source,
            target: DamageTarget::Object(to),
            amount,
            deathtouch: false,
            form: DamageForm::default(),
            toxic: 0,
            unpreventable: false,
        }
    }

    /// The damage dealt and the counters removed by the batch.
    fn outcome(game: &GameState, batch: Vec<Event>) -> (u32, i32) {
        let Processed::Events(events) = process(game, batch, &[]).unwrap() else {
            panic!("nothing is asked");
        };
        events
            .iter()
            .fold((0, 0), |(dealt, removed), event| match event {
                Event::DamageDealt { amount, .. } => (dealt + amount, removed),
                Event::CountersChanged(CountersChanged { delta, .. }) => (dealt, removed - delta),
                _ => (dealt, removed),
            })
    }

    #[rstest]
    #[case::fewer_than_the_counters(2, 3, (0, 2))]
    #[case::as_many_as_the_counters(3, 3, (0, 3))]
    #[case::more_than_the_counters(5, 3, (2, 3))]
    #[case::no_counters(2, 0, (2, 0))]
    fn the_damage_is_prevented_one_for_each_counter_removed(
        #[values(2, 3, 4)] seats: usize,
        #[case] amount: u32,
        #[case] counters: i32,
        #[case] expected: (u32, i32),
    ) {
        let game = game(seats, &[HYDRA], counters);
        let (hydra, bear) = (at(&game, 0), at(&game, 1));
        assert_eq!(outcome(&game, vec![hit(bear, hydra, amount)]), expected);
    }

    #[rstest]
    fn damage_dealt_at_once_counts_the_counters_already_removed(#[values(2, 4)] seats: usize) {
        let game = game(seats, &[HYDRA], 3);
        let (hydra, bear) = (at(&game, 0), at(&game, 1));
        // Two sources deal 2 each at once: three counters prevent three of the four damage.
        let out = outcome(&game, vec![hit(bear, hydra, 2), hit(hydra, hydra, 2)]);
        assert_eq!(out, (1, 3));
    }

    #[rstest]
    fn damage_that_cant_be_prevented_still_removes_the_counters(#[values(2, 3)] seats: usize) {
        let game = game(seats, &[HYDRA, UNPREVENTABLE], 3);
        let (hydra, bear) = (at(&game, 0), at(&game, 1));
        // No damage can be prevented: it is dealt in full and two counters still go.
        assert_eq!(outcome(&game, vec![hit(bear, hydra, 2)]), (2, 2));
    }

    #[test]
    fn damage_to_another_creature_is_dealt() {
        let game = game(3, &[HYDRA], 3);
        let (hydra, bear) = (at(&game, 0), at(&game, 1));
        assert_eq!(outcome(&game, vec![hit(hydra, bear, 2)]), (2, 0));
    }
}
