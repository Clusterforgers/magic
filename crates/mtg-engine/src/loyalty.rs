//! Planeswalkers: the loyalty a planeswalker enters with (CR 306.5b), when its loyalty abilities may be activated and what
//! paying their cost does (CR 606), and the loyalty abilities a permanent has.

use mtg_cards::ability::{Ability, LoyaltyCost};
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::abilities::AbilityRef;
use crate::ability_reader::abilities_of;
use crate::choose::Target;
use crate::counters::{CountersChanged, change};
use crate::events::Event;
use crate::object::{GameObject, ObjectRef};
use crate::priority::sorcery_timing;
use crate::state::GameState;
use crate::zones::Zone;

/// The change of counters that gives the planeswalker card `id` its printed loyalty as it enters the battlefield
/// (CR 306.5b), named for the new object the card becomes (CR 400.7).
///
/// It is proposed with the events that move the card, like any other change of counters, so a rule that changes what
/// enters with counters can reach it. `None` for a card that is not a planeswalker, one that is on the battlefield
/// already and one with no printed loyalty.
pub(crate) fn entering(state: &GameState, id: ObjectId) -> Option<Event> {
    let object = state.object(id)?;
    if object.zone() == Zone::Battlefield || !object.base_types().contains(CardType::Planeswalker) {
        return None;
    }
    let loyalty = object.card().loyalty.filter(|loyalty| *loyalty > 0)?;
    Some(Event::CountersChanged(CountersChanged {
        on: Target::Object(ObjectRef {
            id,
            counter: object.counter() + 1,
        }),
        counter: CounterKind::Loyalty,
        delta: i32::try_from(loyalty).ok()?,
    }))
}

/// Whether `player` may activate a loyalty ability costing `cost` of the planeswalker `object` now: at sorcery timing
/// (CR 606.3), when no loyalty ability of it has been activated this turn, and, for a cost that removes counters, when
/// it holds that many (CR 606.6). A `-X` cost waits for the choice of X.
pub(crate) fn can_activate(
    state: &GameState,
    player: PlayerId,
    object: &GameObject,
    cost: LoyaltyCost,
) -> bool {
    if object.zone() != Zone::Battlefield
        || !sorcery_timing(state, player)
        || state.loyalty_spent(object.reference())
    {
        return false;
    }
    match cost {
        LoyaltyCost::Plus(_) | LoyaltyCost::Zero => true,
        LoyaltyCost::Minus(removed) => object
            .permanent()
            .is_some_and(|permanent| permanent.counters_of(&CounterKind::Loyalty) >= removed),
        LoyaltyCost::MinusX => false,
    }
}

/// The change of counters that pays `cost` on `object`; `None` when the cost changes nothing (`0`) or has no number yet.
pub(crate) fn payment(object: &GameObject, cost: LoyaltyCost) -> Option<Event> {
    let delta = cost.change().filter(|delta| *delta != 0)?;
    Some(Event::CountersChanged(CountersChanged {
        on: Target::Object(object.reference()),
        counter: CounterKind::Loyalty,
        delta,
    }))
}

/// Moves the loyalty counters a snapshot lists by `delta`, as paying a cost moved the permanent's own, so the ability
/// remembers its source as it was once the cost was paid (CR 113.7a).
pub(crate) fn after_payment(counters: &mut Vec<(CounterKind, u32)>, delta: i32) {
    let mut held: std::collections::BTreeMap<CounterKind, u32> = counters.drain(..).collect();
    // The cost was payable, so the counters never fall below none; a refused change leaves them as they were.
    change(&mut held, &CounterKind::Loyalty, delta).ok();
    counters.extend(held);
}

/// The loyalty abilities `id` has now, each with its address and cost, in the order of its abilities. Nothing for an
/// object that is not on the battlefield.
pub fn loyalty_abilities(state: &GameState, id: ObjectId) -> Vec<(AbilityRef, LoyaltyCost)> {
    // Only a planeswalker card has them, so a view of every other permanent costs nothing.
    if !state.object(id).is_some_and(|object| {
        object.zone() == Zone::Battlefield && object.base_types().contains(CardType::Planeswalker)
    }) {
        return Vec::new();
    }
    abilities_of(state, id)
        .into_iter()
        .filter_map(|(address, rules)| match &*rules {
            Ability::Loyalty { cost, .. } => Some((address, *cost)),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: a planeswalker card enters with its printed loyalty as a proposed change of counters for the new object (never for another card, a permanent already in play or a card with no loyalty), a loyalty ability is activated only at sorcery timing, once for each planeswalker each turn, and a cost that removes counters only from a planeswalker that holds that many, paying a cost moves the counters of a snapshot as it moves the permanent's, and a permanent lists the loyalty abilities it has with their costs.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::turn::Step;

    const ME: PlayerId = PlayerId(0);

    fn walker(loyalty: Option<u32>) -> Arc<CardDefinition> {
        let card = CardDefinition::new("Walker")
            .with_type(CardType::Planeswalker)
            .with_rules(
                serde_json::from_str(r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"Draw":{}}]}}"#)
                    .unwrap(),
            )
            .with_rules(
                serde_json::from_str(r#"{"Loyalty":{"cost":{"Minus":3},"effects":[{"Draw":{}}]}}"#)
                    .unwrap(),
            );
        Arc::new(match loyalty {
            Some(loyalty) => card.with_loyalty(loyalty),
            None => card,
        })
    }

    fn walker_in_play(loyalty: u32) -> GameState {
        state(
            vec![vec![], vec![]],
            vec![vec![walker(Some(loyalty))], vec![]],
        )
    }

    fn walker_object(game: &GameState) -> &GameObject {
        game.object(game.zone(Zone::Battlefield)[0]).unwrap()
    }

    #[rstest]
    #[case::three(Some(3), Some(3))]
    #[case::one(Some(1), Some(1))]
    #[case::none_printed(None, None)]
    #[case::zero_printed(Some(0), None)]
    fn a_planeswalker_card_enters_with_its_printed_loyalty(
        #[case] printed: Option<u32>,
        #[case] delta: Option<i32>,
    ) {
        let game = state(vec![vec![walker(printed)], vec![]], vec![vec![], vec![]]);
        let card = game.zone(Zone::Hand(ME))[0];
        let counter = game.object(card).unwrap().counter();
        assert_eq!(
            entering(&game, card),
            delta.map(|delta| Event::CountersChanged(CountersChanged {
                on: Target::Object(ObjectRef {
                    id: card,
                    counter: counter + 1
                }),
                counter: CounterKind::Loyalty,
                delta,
            }))
        );
    }

    #[test]
    fn only_a_planeswalker_card_that_is_not_in_play_enters_with_loyalty() {
        let game = state(
            vec![vec![creature("Bear", 2, 2)], vec![]],
            vec![vec![walker(Some(4))], vec![]],
        );
        assert_eq!(entering(&game, game.zone(Zone::Hand(ME))[0]), None);
        assert_eq!(entering(&game, game.zone(Zone::Battlefield)[0]), None);
    }

    #[rstest]
    #[case::plus_at_sorcery_timing(LoyaltyCost::Plus(2), 3, Step::Main1, false, true)]
    #[case::zero_at_sorcery_timing(LoyaltyCost::Zero, 1, Step::Main2, false, true)]
    #[case::minus_with_exactly_enough(LoyaltyCost::Minus(3), 3, Step::Main1, false, true)]
    #[case::minus_with_too_few(LoyaltyCost::Minus(3), 2, Step::Main1, false, false)]
    #[case::minus_with_more_than_enough(LoyaltyCost::Minus(1), 5, Step::Main1, false, true)]
    #[case::minus_x_waits_for_x(LoyaltyCost::MinusX, 5, Step::Main1, false, false)]
    #[case::not_in_a_main_step(LoyaltyCost::Plus(1), 3, Step::Upkeep, false, false)]
    #[case::not_in_combat(LoyaltyCost::Plus(1), 3, Step::DeclareAttackers, false, false)]
    #[case::not_in_the_end_step(LoyaltyCost::Zero, 3, Step::End, false, false)]
    #[case::not_twice_in_a_turn(LoyaltyCost::Plus(1), 3, Step::Main1, true, false)]
    fn a_loyalty_ability_is_activated_at_sorcery_timing_once_with_the_counters_its_cost_needs(
        #[case] cost: LoyaltyCost,
        #[case] loyalty: u32,
        #[case] step: Step,
        #[case] already_used: bool,
        #[case] allowed: bool,
    ) {
        let mut game = walker_in_play(loyalty);
        apply_event(&mut game, &Event::StepBegan { step }).unwrap();
        if already_used {
            let named = walker_object(&game).reference();
            game.loyalty_used.insert(named);
        }
        assert_eq!(can_activate(&game, ME, walker_object(&game), cost), allowed);
    }

    #[test]
    fn only_the_active_player_activates_a_loyalty_ability() {
        let game = walker_in_play(4);
        assert!(can_activate(
            &game,
            ME,
            walker_object(&game),
            LoyaltyCost::Zero
        ));
        assert!(!can_activate(
            &game,
            PlayerId(1),
            walker_object(&game),
            LoyaltyCost::Zero
        ));
    }

    #[rstest]
    #[case::plus(LoyaltyCost::Plus(2), Some(2))]
    #[case::minus(LoyaltyCost::Minus(3), Some(-3))]
    #[case::zero(LoyaltyCost::Zero, None)]
    #[case::minus_x(LoyaltyCost::MinusX, None)]
    fn paying_a_cost_changes_the_loyalty_counters_by_its_sign(
        #[case] cost: LoyaltyCost,
        #[case] delta: Option<i32>,
    ) {
        let game = walker_in_play(4);
        let paid = payment(walker_object(&game), cost);
        assert_eq!(
            paid,
            delta.map(|delta| Event::CountersChanged(CountersChanged {
                on: Target::Object(walker_object(&game).reference()),
                counter: CounterKind::Loyalty,
                delta,
            }))
        );
    }

    #[rstest]
    #[case::up(&[(CounterKind::Loyalty, 3)], 2, &[(CounterKind::Loyalty, 5)])]
    #[case::down(&[(CounterKind::Loyalty, 3)], -2, &[(CounterKind::Loyalty, 1)])]
    #[case::to_none(&[(CounterKind::Loyalty, 3)], -3, &[])]
    #[case::from_none(&[], 2, &[(CounterKind::Loyalty, 2)])]
    #[case::beside_other_kinds(
        &[(CounterKind::PlusOnePlusOne, 1), (CounterKind::Charge, 2)],
        1,
        &[(CounterKind::PlusOnePlusOne, 1), (CounterKind::Loyalty, 1), (CounterKind::Charge, 2)]
    )]
    fn a_snapshot_moves_its_loyalty_as_the_cost_moved_the_permanents(
        #[case] before: &[(CounterKind, u32)],
        #[case] delta: i32,
        #[case] after: &[(CounterKind, u32)],
    ) {
        let mut counters = before.to_vec();
        after_payment(&mut counters, delta);
        assert_eq!(counters, after);
    }

    #[test]
    fn a_permanent_lists_its_loyalty_abilities_with_their_costs_and_a_card_elsewhere_none() {
        let game = state(
            vec![vec![walker(Some(3))], vec![]],
            vec![vec![walker(Some(3))], vec![]],
        );
        let on_battlefield = game.zone(Zone::Battlefield)[0];
        let costs: Vec<LoyaltyCost> = loyalty_abilities(&game, on_battlefield)
            .into_iter()
            .map(|(_, cost)| cost)
            .collect();
        assert_eq!(costs, [LoyaltyCost::Plus(1), LoyaltyCost::Minus(3)]);
        assert!(loyalty_abilities(&game, game.zone(Zone::Hand(ME))[0]).is_empty());
    }
}
