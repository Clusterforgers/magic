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
/// prints replaced by {0} while a `FreeActivation` of its kind that `player` controls applies; else the printed cost. The
/// mana cost of another object it names ("Pay enchanted creature's mana cost") is that object's mana cost now.
pub(crate) fn cost<'a>(
    state: &GameState,
    player: PlayerId,
    (address, rules): (AbilityRef, &Ability),
    cost: &'a Cost,
) -> Cow<'a, Cost> {
    if cost
        .0
        .iter()
        .any(|item| matches!(item, mtg_cards::cost::CostItem::ManaCostOf(_)))
    {
        let read = r113_mana_cost_of(state, player, address, cost);
        return Cow::Owned(keyworded_cost(state, player, (address, rules), &read).into_owned());
    }
    keyworded_cost(state, player, (address, rules), cost)
}

/// The cost with each `ManaCostOf` part read as the mana cost of the first object it names, seen from the ability's
/// object; an object with no mana cost costs nothing (CR 202.1b).
fn r113_mana_cost_of(
    state: &GameState,
    player: PlayerId,
    address: AbilityRef,
    cost: &Cost,
) -> Cost {
    let mut read = cost.clone();
    let Some(source) = state
        .object(address.object)
        .map(crate::object::GameObject::reference)
    else {
        return read;
    };
    let scope = Scope::announcing(state, player, source);
    for item in &mut read.0 {
        if let mtg_cards::cost::CostItem::ManaCostOf(of) = item {
            let mana = scope
                .objects(of)
                .ok()
                .and_then(|named| named.into_iter().find_map(|one| state.object(one.id)))
                .and_then(|object| object.mana_cost().cloned())
                .unwrap_or(mtg_core::mana::ManaCost::ZERO);
            *item = mtg_cards::cost::CostItem::Mana(ManaCostX::new(0, mana));
        }
    }
    read
}

/// [`cost`] once each part naming another object is read.
fn keyworded_cost<'a>(
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
    // A power-up ability is no keyword: its whole cost is the power-up cost.
    if kind == ActivationKind::PowerUp && matches!(rules, Ability::Activated { .. }) {
        return Cow::Owned(Cost::mana(ManaCostX::default()));
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
pub(crate) fn applies(state: &GameState, player: PlayerId, kind: ActivationKind) -> bool {
    held(state, player, |ability| match ability {
        Ability::FreeActivation {
            kind: wanted,
            condition,
        } => (*wanted == kind).then_some(condition.as_ref()),
        _ => None,
    })
}

/// Whether a permanent `player` controls has an `InstantActivation` of the kind whose condition holds for them: a special
/// action of the kind (foretelling) is open to them any time they have priority.
pub(crate) fn r110_instant(state: &GameState, player: PlayerId, kind: ActivationKind) -> bool {
    held(state, player, |ability| match ability {
        Ability::InstantActivation {
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
