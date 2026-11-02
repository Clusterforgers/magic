//! Smart stops: whether a permanent of a player has an ability the timing lets it use now, read from public facts only.
//!
//! The server makes such a step a priority window for a seat that asks for smart stops. It must not read a hidden zone:
//! the costs are left out, since paying one can read the hand (a discard), and so is everything outside the
//! battlefield. Whether the seat can pay is for the window itself, where a dwell hides it.

use mtg_cards::ability::{Ability, Limit, Timing};
use mtg_core::ids::PlayerId;

use crate::abilities::AbilityRef;
use crate::ability_reader::abilities_of;
use crate::characteristics::{characteristics, controller_of_object};
use crate::keywords;
use crate::object::GameObject;
use crate::price::Price;
use crate::priority::sorcery_timing;
use crate::resolve::{Scope, zone_kind};
use crate::state::GameState;
use crate::zones::Zone;

/// Whether `player` controls a permanent with an ability, not a mana ability, that it may activate now whatever it costs.
pub fn can_answer_now(state: &GameState, player: PlayerId) -> bool {
    // Split second: no ability that is not a mana ability is activated (CR 702.61a).
    if keywords::split_second_on_stack(state) {
        return false;
    }
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| controller_of_object(state, object) == player)
        .any(|object| {
            abilities_of(state, object.id())
                .iter()
                .any(|(address, rules)| timely(state, player, object, *address, rules))
        })
}

/// Whether the timing, the zone, a once-per-turn limit, an "activate only" condition and a `{T}` let `rules` be activated.
fn timely(
    state: &GameState,
    player: PlayerId,
    object: &GameObject,
    address: AbilityRef,
    rules: &Ability,
) -> bool {
    let Ability::Activated {
        cost,
        timing,
        condition,
        limit,
        from,
        ..
    } = rules
    else {
        // A loyalty ability is used at sorcery speed only, which is never a smart stop.
        return false;
    };
    if rules.is_mana_ability() || *from != zone_kind(object.zone()) {
        return false;
    }
    if *timing == Timing::Sorcery && !sorcery_timing(state, player) {
        return false;
    }
    if *limit == Some(Limit::OncePerTurn)
        && state.limit_spent(object.reference(), address.source, address.ability)
    {
        return false;
    }
    if let Some(condition) = condition
        && !Scope::announcing(state, player, object.reference())
            .holds(condition)
            .unwrap_or(false)
    {
        return false;
    }
    let Ok(price) = Price::announced(cost) else {
        return false;
    };
    if price.tap {
        let Some(permanent) = object.permanent() else {
            return false;
        };
        // Only creatures are subject to summoning sickness for {T} abilities (CR 302.6).
        let sick = keywords::summoning_sick(state, object.id())
            && characteristics(state, object.id()).is_some_and(|c| c.is_creature());
        return !permanent.tapped() && !sick;
    }
    true
}
