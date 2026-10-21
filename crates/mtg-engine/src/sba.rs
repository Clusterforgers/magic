//! State-based actions (CR 704), applied in passes until nothing changes.

use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;

use crate::characteristics::{Characteristics, characteristics};
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::{DeathCause, Event};
use crate::keywords;
use crate::object::PermanentState;
use crate::player::{LossReason, Player};
use crate::state::GameState;
use crate::zones::Zone;

/// How many poison counters make a player lose the game (CR 704.5c).
const POISON_LOSS: u32 = 10;

fn loss_reason(state: &GameState, player: &Player) -> Option<LossReason> {
    if player.life() <= 0 {
        Some(LossReason::ZeroLife)
    } else if player.drew_from_empty {
        Some(LossReason::EmptyLibrary)
    } else if player.counters_of(&CounterKind::Poison) >= POISON_LOSS {
        Some(LossReason::Poison)
    } else {
        state.format().loss_condition(state, player.id())
    }
}

/// One simultaneous pass over players and creatures; empty once the state is stable.
pub(crate) fn check(state: &GameState) -> Vec<Event> {
    let mut events: Vec<Event> = state
        .players()
        .iter()
        .filter(|p| p.is_in_game())
        .filter_map(|p| {
            loss_reason(state, p).map(|reason| Event::PlayerLost {
                player: p.id(),
                reason,
            })
        })
        .collect();
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id) else {
            continue;
        };
        if !state.is_alive(object.controller()) {
            continue;
        }
        let Some(chars) = characteristics(state, id).filter(Characteristics::is_creature) else {
            continue;
        };
        let toughness = chars.toughness.unwrap_or(0);
        let damage = object.permanent().map_or(0, PermanentState::damage);
        let deathtouched = object.permanent().is_some_and(PermanentState::deathtouched);
        let Some(cause) = death_cause(toughness, damage, deathtouched, || {
            keywords::is_indestructible(state, id)
        }) else {
            continue;
        };
        events.push(Event::CreatureDied { object: id, cause });
    }
    let died: Vec<ObjectId> = events
        .iter()
        .filter_map(|event| match event {
            Event::CreatureDied { object, .. } => Some(*object),
            _ => None,
        })
        .collect();
    events.extend(annihilation(state, &died));
    events
}

/// A permanent with both a +1/+1 and a -1/-1 counter on it loses as many of each as it has of the kind it has fewer of
/// (CR 704.5q); one that dies in the same pass takes its counters with it.
fn annihilation(state: &GameState, died: &[ObjectId]) -> Vec<Event> {
    let mut events = Vec::new();
    if !state.possible.counters {
        return events;
    }
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state
            .object(id)
            .filter(|o| state.is_alive(o.owner()) && !died.contains(&id))
        else {
            continue;
        };
        let Some(permanent) = object.permanent() else {
            continue;
        };
        let pairs = permanent
            .counters_of(&CounterKind::PlusOnePlusOne)
            .min(permanent.counters_of(&CounterKind::MinusOneMinusOne));
        let Ok(pairs) = i32::try_from(pairs) else {
            continue;
        };
        if pairs > 0 {
            for counter in [CounterKind::PlusOnePlusOne, CounterKind::MinusOneMinusOne] {
                events.push(Event::CountersChanged(CountersChanged {
                    on: Target::Object(object.reference()),
                    counter,
                    delta: -pairs,
                }));
            }
        }
    }
    events
}

/// Why a creature with this toughness, damage marked and deathtouch mark dies now, if it does.
///
/// Zero toughness kills whatever else is true (CR 704.5f). Lethal damage and a mark from deathtouch destroy
/// it (CR 704.5g, 704.5h) unless it is indestructible, which is asked only when one of them applies.
pub(crate) fn death_cause(
    toughness: i32,
    damage: u32,
    deathtouched: bool,
    indestructible: impl FnOnce() -> bool,
) -> Option<DeathCause> {
    if toughness <= 0 {
        return Some(DeathCause::ZeroToughness);
    }
    let lethal = i64::from(damage) >= i64::from(toughness);
    if !(lethal || deathtouched) || indestructible() {
        return None;
    }
    Some(if lethal {
        DeathCause::LethalDamage
    } else {
        DeathCause::Deathtouch
    })
}

/// Housekeeping that follows stable passes: remove eliminated players' objects, drop ended effects and delayed triggers.
pub(crate) fn housekeeping(state: &GameState) -> Vec<Event> {
    let departing: Vec<Event> = state
        .objects()
        .filter(|o| !state.is_alive(o.owner()))
        .map(|o| Event::ObjectLeftGame { object: o.id() })
        .collect();
    if !departing.is_empty() {
        return departing;
    }
    let mut expired: Vec<Event> = state
        .effects()
        .iter()
        .filter(|e| !e.is_active(state))
        .map(|e| Event::EffectExpired { id: e.id })
        .collect();
    expired.extend(
        state
            .rule_effects()
            .iter()
            .filter(|e| !e.is_active(state))
            .map(|e| Event::RuleExpired { id: e.id }),
    );
    expired.extend(
        state
            .delayed()
            .iter()
            .filter(|d| d.lapsed(state))
            .map(|d| Event::DelayedTriggerExpired { id: d.id() }),
    );
    expired
}

#[cfg(test)]
mod tests {
    //! Goal: a creature dies to state-based actions for zero toughness always, and for lethal damage or a deathtouch mark unless it is indestructible, and the cause named is the one that applies; a permanent with both +1/+1 and -1/-1 counters loses a pair of each until it has only one kind; a player with ten or more poison counters loses, and life is named first.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};

    #[rstest]
    #[case::healthy(2, 1, false, false, None)]
    #[case::exactly_lethal(2, 2, false, false, Some(DeathCause::LethalDamage))]
    #[case::more_than_lethal(2, 9, false, false, Some(DeathCause::LethalDamage))]
    #[case::deathtouch_with_one_damage(5, 1, true, false, Some(DeathCause::Deathtouch))]
    #[case::deathtouch_and_lethal_is_lethal_damage(
        2,
        3,
        true,
        false,
        Some(DeathCause::LethalDamage)
    )]
    #[case::zero_toughness(0, 0, false, false, Some(DeathCause::ZeroToughness))]
    #[case::negative_toughness(-1, 0, false, false, Some(DeathCause::ZeroToughness))]
    #[case::indestructible_shrugs_off_lethal_damage(2, 9, false, true, None)]
    #[case::indestructible_shrugs_off_deathtouch(2, 1, true, true, None)]
    #[case::indestructible_still_dies_to_zero_toughness(
        0,
        0,
        false,
        true,
        Some(DeathCause::ZeroToughness)
    )]
    #[case::indestructible_with_negative_toughness(
        -3,
        5,
        true,
        true,
        Some(DeathCause::ZeroToughness)
    )]
    fn the_cause_of_death(
        #[case] toughness: i32,
        #[case] damage: u32,
        #[case] deathtouched: bool,
        #[case] indestructible: bool,
        #[case] expected: Option<DeathCause>,
    ) {
        assert_eq!(
            death_cause(toughness, damage, deathtouched, || indestructible),
            expected
        );
    }

    #[test]
    fn indestructibility_is_asked_only_when_damage_would_destroy() {
        let ask = || -> bool { unreachable!("not asked") };
        assert_eq!(death_cause(3, 2, false, ask), None);
        assert_eq!(
            death_cause(0, 0, false, ask),
            Some(DeathCause::ZeroToughness)
        );
    }

    const PLUS: CounterKind = CounterKind::PlusOnePlusOne;
    const MINUS: CounterKind = CounterKind::MinusOneMinusOne;

    /// Seat 0 controls a Bear with these counters and has this many poison counters; seat 1 controls an Ogre.
    fn game_with(counters: &[(CounterKind, u32)], poison: u32) -> GameState {
        let mut game = setup(
            vec![vec![], vec![]],
            vec![vec![creature("Bear", 2, 2)], vec![creature("Ogre", 3, 3)]],
        );
        game.players[0].battlefield[0].counters = counters.to_vec();
        game.players[0].counters = vec![(CounterKind::Poison, poison)];
        GameState::from_setup(&game).unwrap()
    }

    /// The counter changes the check asks for, as (kind, change).
    fn counter_changes(game: &GameState) -> Vec<(CounterKind, i32)> {
        check(game)
            .into_iter()
            .filter_map(|event| match event {
                Event::CountersChanged(changed) => Some((changed.counter, changed.delta)),
                _ => None,
            })
            .collect()
    }

    #[rstest]
    #[case::no_counters(&[], &[])]
    #[case::only_plus_counters(&[(PLUS, 2)], &[])]
    #[case::only_minus_counters(&[(MINUS, 2)], &[])]
    #[case::one_pair(&[(PLUS, 1), (MINUS, 1)], &[(PLUS, -1), (MINUS, -1)])]
    #[case::more_plus_than_minus(&[(PLUS, 3), (MINUS, 1)], &[(PLUS, -1), (MINUS, -1)])]
    #[case::more_minus_than_plus(&[(PLUS, 3), (MINUS, 4)], &[(PLUS, -3), (MINUS, -3)])]
    #[case::as_many_of_each(&[(PLUS, 3), (MINUS, 3)], &[(PLUS, -3), (MINUS, -3)])]
    #[case::other_kinds_stay(&[(PLUS, 1), (CounterKind::Charge, 1)], &[])]
    fn a_permanent_with_both_kinds_loses_a_pair_of_each(
        #[case] counters: &[(CounterKind, u32)],
        #[case] expected: &[(CounterKind, i32)],
    ) {
        assert_eq!(counter_changes(&game_with(counters, 0)), expected);
    }

    #[test]
    fn the_pairs_of_every_permanent_go_together_and_only_while_a_game_has_such_counters() {
        let mut game = setup(
            vec![vec![], vec![]],
            vec![vec![creature("Bear", 2, 2)], vec![creature("Ogre", 3, 3)]],
        );
        game.players[0].battlefield[0].counters = vec![(PLUS, 2), (MINUS, 1)];
        game.players[1].battlefield[0].counters = vec![(PLUS, 1), (MINUS, 2)];
        let game = GameState::from_setup(&game).unwrap();
        assert_eq!(
            counter_changes(&game),
            [(PLUS, -1), (MINUS, -1), (PLUS, -1), (MINUS, -1)]
        );
        assert!(game.possible.counters);
        let plain = game_with(&[], 0);
        assert!(
            !plain.possible.counters,
            "a game without such counters looks for none"
        );
        assert_eq!(check(&plain), []);
    }

    #[test]
    fn a_creature_that_dies_in_the_pass_takes_its_pairs_with_it() {
        // 2/2 with one +1/+1 and three -1/-1 counters: toughness -1.
        let events = check(&game_with(&[(PLUS, 1), (MINUS, 3)], 0));
        assert!(
            matches!(
                events.as_slice(),
                [Event::CreatureDied {
                    cause: DeathCause::ZeroToughness,
                    ..
                }]
            ),
            "{events:?}"
        );
    }

    #[rstest]
    #[case::none(0, false)]
    #[case::nine(9, false)]
    #[case::ten(10, true)]
    #[case::eleven(11, true)]
    fn ten_poison_counters_lose_the_game(#[case] poison: u32, #[case] loses: bool) {
        let events = check(&game_with(&[], poison));
        let lost = Event::PlayerLost {
            player: PlayerId(0),
            reason: LossReason::Poison,
        };
        assert_eq!(events.contains(&lost), loses, "{events:?}");
        assert_eq!(events.len(), usize::from(loses));
    }

    #[test]
    fn a_player_out_of_life_loses_for_that_reason_before_poison() {
        let mut game = setup(vec![vec![], vec![]], vec![vec![], vec![]]);
        game.players[0].life = Some(0);
        game.players[0].counters = vec![(CounterKind::Poison, 12)];
        let events = check(&GameState::from_setup(&game).unwrap());
        assert_eq!(
            events,
            [Event::PlayerLost {
                player: PlayerId(0),
                reason: LossReason::ZeroLife
            }]
        );
    }
}
