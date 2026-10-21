//! Counters on permanents and players: the event that changes them, its fold and the readers of how many there are.

use std::collections::BTreeMap;

use mtg_cards::value::CounterKind;
use serde::{Deserialize, Serialize};

use crate::choose::Target;
use crate::error::EngineError;
use crate::fold::{invalid, player_mut};
use crate::state::GameState;
use crate::zones::Zone;

/// Counters put on or removed from an object or a player.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CountersChanged {
    /// What holds the counters.
    pub on: Target,
    /// Their kind.
    pub counter: CounterKind,
    /// How many were added (positive) or removed (negative).
    pub delta: i32,
}

/// Whether counters of this kind change the power and toughness of the permanent that holds them (CR 613.4c).
pub(crate) const fn changes_power_toughness(kind: &CounterKind) -> bool {
    matches!(
        kind,
        CounterKind::PlusOnePlusOne | CounterKind::MinusOneMinusOne
    )
}

/// Folds the event. Only a permanent on the battlefield, the very object that was named (CR 400.7), and a player who is
/// still in the game hold counters; a change of nothing, and a removal of more counters than there are, cannot happen.
pub(crate) fn fold(state: &mut GameState, event: &CountersChanged) -> Result<(), EngineError> {
    match event.on {
        Target::Object(named) => {
            let object = state
                .objects
                .get_mut(&named.id)
                .filter(|object| object.counter() == named.counter)
                .ok_or_else(|| invalid(format!("{named:?} is not in the game")))?;
            let counters = if let Some(permanent) = object.permanent.as_mut() {
                &mut permanent.counters
            } else if object.zone == Zone::Exile {
                &mut object.card_counters
            } else {
                return Err(invalid(format!(
                    "{named:?} is not a permanent on the battlefield or a card in exile"
                )));
            };
            change(counters, &event.counter, event.delta)?;
            if event.delta > 0 && changes_power_toughness(&event.counter) {
                state.possible.counters = true;
            }
        }
        Target::Player(id) => {
            let player = player_mut(state, id)?;
            if !player.is_in_game() {
                return Err(invalid(format!("{id:?} is out of the game")));
            }
            change(&mut player.counters, &event.counter, event.delta)?;
        }
    }
    Ok(())
}

/// Adds `delta` counters of `kind` to what is held, or takes them off; a kind that falls to none is no longer listed.
pub(crate) fn change(
    held: &mut BTreeMap<CounterKind, u32>,
    kind: &CounterKind,
    delta: i32,
) -> Result<(), EngineError> {
    let by = delta.unsigned_abs();
    if by == 0 {
        return Err(invalid("a change of no counters"));
    }
    let now = held.get(kind).copied().unwrap_or(0);
    let after = if delta > 0 {
        now.checked_add(by)
            .ok_or_else(|| invalid(format!("{by} more {kind:?} counters than can be counted")))?
    } else {
        now.checked_sub(by)
            .ok_or_else(|| invalid(format!("cannot remove {by} {kind:?} counters from {now}")))?
    };
    if after == 0 {
        held.remove(kind);
    } else {
        held.insert(kind.clone(), after);
    }
    Ok(())
}

/// How many counters of `kind` `target` holds now: a player's, or those of the permanent when it is still the very
/// object that was named; none for anything else.
pub(crate) fn held_by(state: &GameState, target: Target, kind: &CounterKind) -> u32 {
    match target {
        Target::Player(id) => state.player(id).map_or(0, |p| p.counters_of(kind)),
        Target::Object(named) => state
            .object(named.id)
            .filter(|object| object.counter() == named.counter)
            .map_or(0, |object| match object.permanent() {
                Some(permanent) => permanent.counters_of(kind),
                None => object.card_counters_of(kind),
            }),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: counters are put on and taken off permanents and players by the fold, never fewer than none and never on what cannot hold them (a card elsewhere, an object that has since changed zones, a player who is out); a kind that falls to none is no longer listed; counters leave with the object, so a permanent that comes back holds none (CR 400.7); and only +1/+1 and -1/-1 counters make the layers look at the power and toughness.
    use mtg_core::ids::PlayerId;
    use proptest::prelude::*;
    use rstest::rstest;

    use super::*;
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::object::ObjectRef;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    /// Seat 0 holds a Spell and controls a Bear; seat 1 controls an Ogre.
    fn game() -> GameState {
        state(
            vec![vec![creature("Spell", 1, 1)], vec![]],
            vec![vec![creature("Bear", 2, 2)], vec![creature("Ogre", 3, 3)]],
        )
    }

    fn bear(game: &GameState) -> ObjectRef {
        game.object(game.zone(Zone::Battlefield)[0])
            .unwrap()
            .reference()
    }

    fn spell(game: &GameState) -> ObjectRef {
        game.object(game.zone(Zone::Hand(ME))[0])
            .unwrap()
            .reference()
    }

    fn put(on: Target, counter: CounterKind, delta: i32) -> Event {
        Event::CountersChanged(CountersChanged { on, counter, delta })
    }

    fn fold_all(game: &mut GameState, events: &[Event]) {
        for event in events {
            apply_event(game, event).unwrap_or_else(|error| panic!("{event:?}: {error}"));
        }
    }

    fn on_bear(game: &GameState, kind: &CounterKind) -> u32 {
        held_by(game, Target::Object(bear(game)), kind)
    }

    #[rstest]
    #[case::put_one(vec![1], 1)]
    #[case::put_two_in_two_events(vec![1, 1], 2)]
    #[case::put_and_remove_one(vec![3, -1], 2)]
    #[case::put_and_remove_all(vec![3, -3], 0)]
    #[case::remove_and_put_again(vec![2, -2, 1], 1)]
    fn counters_on_a_permanent_add_up_and_come_off(#[case] deltas: Vec<i32>, #[case] left: u32) {
        let mut game = game();
        let on = Target::Object(bear(&game));
        for delta in deltas {
            fold_all(&mut game, &[put(on, CounterKind::Charge, delta)]);
        }
        assert_eq!(on_bear(&game, &CounterKind::Charge), left);
        let listed: Vec<_> = game
            .object(bear(&game).id)
            .unwrap()
            .permanent()
            .unwrap()
            .counters()
            .map(|(k, n)| (k.clone(), n))
            .collect();
        assert_eq!(
            listed.is_empty(),
            left == 0,
            "{listed:?}: a kind with none is not listed"
        );
    }

    #[test]
    fn a_player_holds_counters_of_every_kind_and_the_kinds_are_listed_in_order() {
        let mut game = game();
        let me = Target::Player(ME);
        fold_all(
            &mut game,
            &[
                put(me, CounterKind::Other("experience".into()), 2),
                put(me, CounterKind::Poison, 3),
                put(me, CounterKind::Other("energy".into()), 4),
                put(me, CounterKind::Poison, -1),
            ],
        );
        let listed: Vec<_> = game
            .player(ME)
            .unwrap()
            .counters()
            .map(|(k, n)| (k.clone(), n))
            .collect();
        assert_eq!(
            listed,
            [
                (CounterKind::Poison, 2),
                (CounterKind::Other("energy".into()), 4),
                (CounterKind::Other("experience".into()), 2),
            ]
        );
        assert_eq!(
            held_by(&game, Target::Player(PlayerId(1)), &CounterKind::Poison),
            0
        );
    }

    #[test]
    fn kinds_are_held_apart() {
        let mut game = game();
        let on = Target::Object(bear(&game));
        fold_all(
            &mut game,
            &[
                put(on, CounterKind::PlusOnePlusOne, 2),
                put(on, CounterKind::MinusOneMinusOne, 1),
                put(on, CounterKind::Other("quest".into()), 1),
                put(on, CounterKind::PlusOnePlusOne, -1),
            ],
        );
        assert_eq!(on_bear(&game, &CounterKind::PlusOnePlusOne), 1);
        assert_eq!(on_bear(&game, &CounterKind::MinusOneMinusOne), 1);
        assert_eq!(on_bear(&game, &CounterKind::Other("quest".into())), 1);
        assert_eq!(on_bear(&game, &CounterKind::Other("page".into())), 0);
    }

    #[rstest]
    #[case::a_change_of_nothing(0)]
    #[case::removing_from_none(-1)]
    fn a_change_that_cannot_happen_is_refused(#[case] delta: i32) {
        let mut game = game();
        let event = put(Target::Player(ME), CounterKind::Poison, delta);
        assert!(apply_event(&mut game, &event).is_err(), "{event:?}");
    }

    #[test]
    fn more_counters_than_can_be_counted_are_refused() {
        let mut game = game();
        let me = Target::Player(ME);
        fold_all(
            &mut game,
            &[
                put(me, CounterKind::Charge, i32::MAX),
                put(me, CounterKind::Charge, i32::MAX),
            ],
        );
        assert!(apply_event(&mut game, &put(me, CounterKind::Charge, 2)).is_err());
        assert_eq!(held_by(&game, me, &CounterKind::Charge), u32::MAX - 1);
    }

    #[test]
    fn more_cannot_be_removed_than_there_are_and_the_refusal_changes_nothing() {
        let mut game = game();
        let on = Target::Object(bear(&game));
        fold_all(&mut game, &[put(on, CounterKind::Charge, 2)]);
        assert!(apply_event(&mut game, &put(on, CounterKind::Charge, -3)).is_err());
        assert!(apply_event(&mut game, &put(on, CounterKind::Poison, -1)).is_err());
        assert_eq!(on_bear(&game, &CounterKind::Charge), 2);
    }

    #[test]
    fn a_card_that_is_not_on_the_battlefield_holds_no_counters() {
        let mut game = game();
        let in_hand = Target::Object(spell(&game));
        let event = put(in_hand, CounterKind::Charge, 1);
        assert!(apply_event(&mut game, &event).is_err());
        let gone = Target::Object(ObjectRef {
            id: mtg_core::ids::ObjectId(9999),
            counter: 0,
        });
        assert!(apply_event(&mut game, &put(gone, CounterKind::Charge, 1)).is_err());
    }

    #[test]
    fn a_player_who_is_out_of_the_game_holds_no_counters() {
        let mut game = game();
        fold_all(
            &mut game,
            &[Event::PlayerLost {
                player: PlayerId(1),
                reason: crate::player::LossReason::Conceded,
            }],
        );
        let event = put(Target::Player(PlayerId(1)), CounterKind::Poison, 1);
        assert!(apply_event(&mut game, &event).is_err());
        assert!(
            apply_event(
                &mut game,
                &put(Target::Player(PlayerId(5)), CounterKind::Poison, 1)
            )
            .is_err()
        );
    }

    #[test]
    fn counters_stay_with_the_object_that_held_them() {
        let mut game = game();
        let old = bear(&game);
        let on = Target::Object(old);
        fold_all(
            &mut game,
            &[
                put(on, CounterKind::PlusOnePlusOne, 2),
                Event::ZoneChanged {
                    object: old.id,
                    from: Zone::Battlefield,
                    to: Zone::Hand(ME),
                    cause: ZoneCause::Moved,
                },
            ],
        );
        // In the hand it holds none, and nothing is put on a card there.
        assert_eq!(held_by(&game, on, &CounterKind::PlusOnePlusOne), 0);
        let in_hand = Target::Object(game.object(old.id).unwrap().reference());
        assert!(apply_event(&mut game, &put(in_hand, CounterKind::Charge, 1)).is_err());
        // It comes back as a new object with none, and the old name no longer reaches it.
        fold_all(
            &mut game,
            &[Event::ZoneChanged {
                object: old.id,
                from: Zone::Hand(ME),
                to: Zone::Battlefield,
                cause: ZoneCause::Moved,
            }],
        );
        let back = game.object(old.id).unwrap().reference();
        assert_ne!(back, old);
        assert_eq!(
            held_by(&game, Target::Object(back), &CounterKind::PlusOnePlusOne),
            0
        );
        assert!(
            apply_event(&mut game, &put(on, CounterKind::PlusOnePlusOne, 1)).is_err(),
            "the old incarnation is gone"
        );
        fold_all(
            &mut game,
            &[put(Target::Object(back), CounterKind::PlusOnePlusOne, 1)],
        );
        assert_eq!(
            held_by(&game, Target::Object(back), &CounterKind::PlusOnePlusOne),
            1
        );
    }

    #[rstest]
    #[case::a_plus_counter(CounterKind::PlusOnePlusOne, true)]
    #[case::a_minus_counter(CounterKind::MinusOneMinusOne, true)]
    #[case::a_charge_counter(CounterKind::Charge, false)]
    #[case::a_loyalty_counter(CounterKind::Loyalty, false)]
    #[case::a_named_counter(CounterKind::Other("quest".into()), false)]
    fn only_counters_that_change_power_and_toughness_make_the_layers_look(
        #[case] kind: CounterKind,
        #[case] flagged: bool,
    ) {
        let mut game = game();
        assert!(!game.possible.counters);
        let on = Target::Object(bear(&game));
        fold_all(&mut game, &[put(on, kind, 1)]);
        assert_eq!(game.possible.counters, flagged);
    }

    #[test]
    fn counters_on_a_player_do_not_make_the_layers_look() {
        let mut game = game();
        fold_all(
            &mut game,
            &[put(Target::Player(ME), CounterKind::PlusOnePlusOne, 1)],
        );
        assert!(!game.possible.counters);
    }

    proptest! {
        /// Any run of changes keeps the totals of a model: a change the model allows folds and gives the same totals
        /// whoever holds them, one it does not allow is refused and changes nothing, and no kind with none is listed.
        #[test]
        fn the_fold_keeps_the_totals_of_a_model_and_refuses_what_the_model_refuses(
            changes in proptest::collection::vec((0usize..3, any::<bool>(), -4i32..=4), 0..60)
        ) {
            let kinds = [
                CounterKind::Poison,
                CounterKind::Other("energy".into()),
                CounterKind::PlusOnePlusOne,
            ];
            let mut game = game();
            let holders = [Target::Player(ME), Target::Object(bear(&game))];
            let mut model = [[0u32; 3]; 2];
            for (kind, on_the_bear, delta) in changes {
                let holder = usize::from(on_the_bear);
                let event = put(holders[holder], kinds[kind].clone(), delta);
                let allowed =
                    delta > 0 || (delta < 0 && model[holder][kind] >= delta.unsigned_abs());
                let before = game.state_hash();
                let folded = apply_event(&mut game, &event).is_ok();
                prop_assert_eq!(folded, allowed, "{:?}", event);
                if folded {
                    model[holder][kind] = model[holder][kind].checked_add_signed(delta).unwrap();
                } else {
                    prop_assert_eq!(game.state_hash(), before);
                }
                for (at, holds) in holders.iter().enumerate() {
                    for (k, counter) in kinds.iter().enumerate() {
                        prop_assert_eq!(held_by(&game, *holds, counter), model[at][k]);
                    }
                }
            }
            let listed = game.player(ME).unwrap().counters().count();
            prop_assert_eq!(listed, model[0].iter().filter(|n| **n > 0).count());
            let on_the_bear = game.object(bear(&game).id).unwrap().permanent().unwrap().counters().count();
            prop_assert_eq!(on_the_bear, model[1].iter().filter(|n| **n > 0).count());
        }
    }
}
