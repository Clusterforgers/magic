//! "You may pay {0} rather than pay the equip cost of the first equip ability you activate each turn." (CR 118.9): the
//! cost of an activation of a kind with a `FreeActivation` in play, and the turn's tally of activations by kind that its
//! condition reads ("the first ... each turn"); and "you may activate equip abilities any time you could cast an instant"
//! (`InstantActivation`).
//!
//! The alternative is always taken: paying {0} in place of the printed cost never leaves the player worse off, and the
//! activation counts toward "the first" either way.

use std::borrow::Cow;

use mtg_cards::ability::{Ability, ActivationKind};
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_cards::mana::ManaCostX;
use mtg_core::ids::PlayerId;

use crate::abilities::AbilityRef;
use crate::ability_reader::{
    abilities_of, definition_has_cost_modifier, is_cost_modifier, keyword_adding, may_hold,
};
use crate::characteristics::controller_of_object;
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// The cost `player` pays to activate the ability `rules` at `address`, whose printed cost is `cost`: the cost its keyword
/// prints replaced by {0} while a `FreeActivation` of its kind that `player` controls applies; else the printed cost.
pub(crate) fn cost<'a>(
    state: &GameState,
    player: PlayerId,
    (address, rules): (AbilityRef, &Ability),
    cost: &'a Cost,
) -> Cow<'a, Cost> {
    if !state.possible.costs {
        return Cow::Borrowed(cost);
    }
    let Some(kind) = crate::activation_kind::specific(state, address, rules) else {
        return Cow::Borrowed(cost);
    };
    if !applies(state, player, kind) {
        return Cow::Borrowed(cost);
    }
    keyword_adding(state, address)
        .and_then(|keyword| at_zero(&keyword))
        .and_then(|free| {
            crate::keywords::rules(&free)
                .adds_abilities(&free)
                .into_iter()
                .nth(address.ability)
        })
        .and_then(|ability| match ability {
            Ability::Activated { cost, .. } => Some(Cow::Owned(cost)),
            _ => None,
        })
        .unwrap_or(Cow::Borrowed(cost))
}

/// Whether `player` may activate the ability `rules` at `address`, which is activated only as a sorcery, now: they could
/// cast a sorcery, or an `InstantActivation` lets them at instant speed.
pub(crate) fn timing_open(
    state: &GameState,
    player: PlayerId,
    (address, rules): (AbilityRef, &Ability),
) -> bool {
    crate::priority::sorcery_timing(state, player)
        || at_instant_speed(state, player, (address, rules))
}

/// Whether `player` may activate the ability `rules` at `address`, which is activated only as a sorcery, any time they
/// could cast an instant: a permanent they control has an `InstantActivation` of its kind whose condition holds.
fn at_instant_speed(
    state: &GameState,
    player: PlayerId,
    (address, rules): (AbilityRef, &Ability),
) -> bool {
    crate::activation_kind::specific(state, address, rules).is_some_and(|kind| {
        held(state, player, |ability| match ability {
            Ability::InstantActivation {
                kind: wanted,
                condition,
            } => (*wanted == kind).then_some(condition.as_ref()),
            _ => None,
        })
    })
}

/// Whether a permanent `player` controls has a `FreeActivation` of the kind whose condition holds for them.
fn applies(state: &GameState, player: PlayerId, kind: ActivationKind) -> bool {
    held(state, player, |ability| match ability {
        Ability::FreeActivation {
            kind: wanted,
            condition,
        } => (*wanted == kind).then_some(condition.as_ref()),
        _ => None,
    })
}

/// Whether a permanent `player` controls has an ability `wanted` picks (with the condition it holds under, none for
/// always) whose condition holds for them.
fn held(
    state: &GameState,
    player: PlayerId,
    wanted: impl Fn(&Ability) -> Option<Option<&mtg_cards::value::Condition>>,
) -> bool {
    state.zone(Zone::Battlefield).iter().any(|id| {
        let Some(holder) = state.object(*id) else {
            return false;
        };
        if controller_of_object(state, holder) != player
            || !may_hold(
                state,
                holder,
                definition_has_cost_modifier,
                is_cost_modifier,
            )
        {
            return false;
        }
        abilities_of(state, *id).iter().any(|(_, ability)| {
            wanted(ability).is_some_and(|condition| {
                condition.is_none_or(|held| {
                    Scope::announcing(state, player, holder.reference())
                        .holds(held)
                        .unwrap_or(false)
                })
            })
        })
    })
}

/// The keyword with its cost replaced by {0}; none for a keyword whose cost no `FreeActivation` replaces.
fn at_zero(keyword: &Keyword) -> Option<Keyword> {
    let zero = Cost::mana(ManaCostX::default());
    Some(match keyword {
        Keyword::Equip(_) => Keyword::Equip(zero),
        Keyword::Fortify(_) => Keyword::Fortify(zero),
        Keyword::EquipTo { to, .. } => Keyword::EquipTo {
            to: to.clone(),
            cost: zero,
        },
        Keyword::Cycling(_) => Keyword::Cycling(zero),
        Keyword::Typecycling { find, .. } => Keyword::Typecycling {
            find: find.clone(),
            cost: zero,
        },
        Keyword::Unearth(_) => Keyword::Unearth(zero),
        _ => return None,
    })
}
