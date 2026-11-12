//! Replacements whose number is worked out as the event happens (`ReplaceBy::AddAmount`, `ReplaceBy::PreventAmount`)
//! or that apply only to a small enough amount of damage (`ReplaceBy::PreventAtMost`): each becomes the fixed
//! replacement it stands for when it is offered for an event, read by its source's controller (CR 614.1, 615).

use mtg_cards::trigger::ReplaceBy;

use super::{Item, Pass, Source};
use crate::error::EngineError;
use crate::events::Event;
use crate::resolve::Scope;

impl Pass<'_> {
    /// The replacement `by` stands for at the event in slot `at`, or none when it does not apply to it.
    pub(super) fn r304c_fixed(
        &self,
        at: usize,
        by: &ReplaceBy,
        source: &Source,
    ) -> Result<Option<ReplaceBy>, EngineError> {
        let worked = |amount| -> Result<u32, EngineError> {
            let scope = Scope::announcing(self.state, source.controller, source.object);
            Ok(u32::try_from(scope.amount(amount)?.max(0)).unwrap_or(u32::MAX))
        };
        Ok(match by {
            ReplaceBy::AddAmount(amount) => match worked(amount)? {
                0 => None,
                more => Some(ReplaceBy::Add(more)),
            },
            ReplaceBy::PreventAmount(amount) => match worked(amount)? {
                0 => None,
                most => Some(ReplaceBy::PreventUpTo(most)),
            },
            ReplaceBy::PreventAtMost(most) => match &self.slots[at].item {
                Item::One(Event::DamageDealt { amount, .. }) if amount <= most => {
                    Some(ReplaceBy::Prevent)
                }
                _ => None,
            },
            other => Some(other.clone()),
        })
    }
}

#[cfg(test)]
mod tests {
    //! Goal: an increase or prevention worked out as the event happens changes the damage by the amount its source's
    //! controller reads then (and nothing when it reads 0), a prevention of small damage prevents only damage of at most
    //! its number, and a lessening of counters puts one fewer; at two, three and four seats.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
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

    /// Damage to a creature is increased by the number of creatures on the battlefield.
    const PLUS_CREATURES: &str = r#"{"Replacement":{"when":{"Damage":{"to":{"Each":[{"Type":"Creature"}]}}},"by":{"AddAmount":{"Count":[{"Type":"Creature"}]}}}}"#;
    /// The same by the number of planeswalkers, which is 0.
    const PLUS_NOTHING: &str = r#"{"Replacement":{"when":{"Damage":{"to":{"Each":[{"Type":"Creature"}]}}},"by":{"AddAmount":{"Count":[{"Type":"Planeswalker"}]}}}}"#;
    /// Damage to a creature you control loses up to the number of creatures on the battlefield.
    const PREVENT_CREATURES: &str = r#"{"Replacement":{"when":{"Damage":{"to":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}},"by":{"PreventAmount":{"Count":[{"Type":"Creature"}]}}}}"#;
    /// Damage of 3 or less to a creature you control is prevented.
    const AT_MOST_THREE: &str = r#"{"Replacement":{"when":{"Damage":{"to":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}},"by":{"PreventAtMost":3}}}"#;
    /// -1/-1 counters put on a creature you control are one fewer.
    const ONE_FEWER: &str = r#"{"Replacement":{"when":{"CountersPut":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"-1/-1"}},"by":{"Subtract":1}}}"#;

    /// Seat 0 controls a Source with `rule` and a Wall; seat 1 controls a Bear; the other seats are empty.
    fn game(seats: usize, rule: &str) -> GameState {
        let rule: Ability = serde_json::from_str(rule).unwrap();
        let source = CardDefinition::new("Source")
            .with_type(CardType::Enchantment)
            .with_rules(rule);
        let mut battlefields = vec![
            vec![Arc::new(source), creature("Wall", 0, 9)],
            vec![creature("Bear", 2, 9)],
        ];
        battlefields.resize(seats, Vec::new());
        state(vec![vec![]; seats], battlefields)
    }

    fn at(game: &GameState, index: usize) -> ObjectId {
        game.zone(Zone::Battlefield)[index]
    }

    fn events(game: &GameState, batch: Vec<Event>) -> Vec<Event> {
        match process(game, batch, &[]).unwrap() {
            Processed::Events(events) => events,
            Processed::Ask { .. } => panic!("nothing is asked"),
        }
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

    /// The damage the events deal in all.
    fn dealt(out: &[Event]) -> u32 {
        out.iter()
            .map(|event| match event {
                Event::DamageDealt { amount, .. } => *amount,
                _ => 0,
            })
            .sum()
    }

    #[rstest]
    fn damage_grows_by_the_amount_read_as_it_happens(
        #[values(2, 3, 4)] seats: usize,
        #[values(1, 3)] amount: u32,
    ) {
        let game = game(seats, PLUS_CREATURES);
        let (wall, bear) = (at(&game, 1), at(&game, 2));
        // Two creatures are on the battlefield.
        assert_eq!(
            dealt(&events(&game, vec![hit(wall, bear, amount)])),
            amount + 2
        );
        let plain = hit(wall, bear, amount);
        assert_eq!(
            events(&self::game(seats, PLUS_NOTHING), vec![plain.clone()]),
            [plain]
        );
    }

    #[rstest]
    #[case::all(1, 0)]
    #[case::two_of_three(3, 1)]
    #[case::two_of_five(5, 3)]
    fn up_to_the_amount_read_is_prevented(
        #[values(2, 3, 4)] seats: usize,
        #[case] amount: u32,
        #[case] left: u32,
    ) {
        let game = game(seats, PREVENT_CREATURES);
        let (wall, bear) = (at(&game, 1), at(&game, 2));
        assert_eq!(dealt(&events(&game, vec![hit(bear, wall, amount)])), left);
        // Damage to another player's creature is dealt whole.
        assert_eq!(dealt(&events(&game, vec![hit(wall, bear, amount)])), amount);
    }

    #[rstest]
    #[case::one(1, 0)]
    #[case::three(3, 0)]
    #[case::four(4, 4)]
    #[case::seven(7, 7)]
    fn only_small_enough_damage_is_prevented(
        #[values(2, 3, 4)] seats: usize,
        #[case] amount: u32,
        #[case] left: u32,
    ) {
        let game = game(seats, AT_MOST_THREE);
        let (wall, bear) = (at(&game, 1), at(&game, 2));
        assert_eq!(dealt(&events(&game, vec![hit(bear, wall, amount)])), left);
        assert_eq!(dealt(&events(&game, vec![hit(wall, bear, amount)])), amount);
    }

    #[rstest]
    #[case::one(1)]
    #[case::two(2)]
    #[case::five(5)]
    fn counters_put_are_one_fewer(#[values(2, 3, 4)] seats: usize, #[case] count: i32) {
        let game = game(seats, ONE_FEWER);
        let (wall, bear) = (at(&game, 1), at(&game, 2));
        let put = |id: ObjectId, counter: CounterKind| {
            Event::CountersChanged(CountersChanged {
                on: Target::Object(game.object(id).unwrap().reference()),
                counter,
                delta: count,
            })
        };
        let total = |out: &[Event]| -> i32 {
            out.iter()
                .map(|event| match event {
                    Event::CountersChanged(changed) => changed.delta,
                    _ => 0,
                })
                .sum()
        };
        assert_eq!(
            total(&events(
                &game,
                vec![put(wall, CounterKind::MinusOneMinusOne)]
            )),
            count - 1
        );
        // Other kinds and other players' creatures keep their number.
        assert_eq!(
            total(&events(&game, vec![put(wall, CounterKind::PlusOnePlusOne)])),
            count
        );
        assert_eq!(
            total(&events(
                &game,
                vec![put(bear, CounterKind::MinusOneMinusOne)]
            )),
            count
        );
    }
}
