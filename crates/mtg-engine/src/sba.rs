//! State-based actions (CR 704), applied in passes until nothing changes.

use crate::characteristics::{Characteristics, characteristics};
use crate::events::{DeathCause, Event};
use crate::keywords;
use crate::object::PermanentState;
use crate::player::{LossReason, Player};
use crate::state::GameState;
use crate::zones::Zone;

fn loss_reason(state: &GameState, player: &Player) -> Option<LossReason> {
    if player.life() <= 0 {
        Some(LossReason::ZeroLife)
    } else if player.drew_from_empty {
        Some(LossReason::EmptyLibrary)
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
            .delayed()
            .iter()
            .filter(|d| d.lapsed(state))
            .map(|d| Event::DelayedTriggerExpired { id: d.id() }),
    );
    expired
}

#[cfg(test)]
mod tests {
    //! Goal: a creature dies to state-based actions for zero toughness always, and for lethal damage or a deathtouch mark unless it is indestructible, and the cause named is the one that applies.
    use super::*;
    use rstest::rstest;

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
}
