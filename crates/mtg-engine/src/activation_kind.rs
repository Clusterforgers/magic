//! The kind of an activated ability (`ActivationKind`) and the cost modifiers that change the cost of activating abilities
//! of a kind (CR 601.2f, 602.2b): "Equip abilities you activate cost {1} less", "Activated abilities of creatures you
//! control cost {2} less to activate. This effect can't reduce the mana in that cost to less than one mana."

use mtg_cards::ability::{Ability, ActivationKind, CostChange};
use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;

use crate::abilities::AbilityRef;
use crate::ability_reader::{abilities_of, definition_has_cost_modifier, keyword_adding};
use crate::characteristics::controller_of_object;
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// The most times one modifier changes a cost: a count this large is more mana than any game has.
const MOST_TIMES: i64 = 100;

/// The one specific kind the ability `rules`, which `address` names, is: a loyalty ability, the equip ability or the
/// cycling ability; none for any other activated ability.
pub(crate) fn specific(
    state: &GameState,
    address: AbilityRef,
    rules: &Ability,
) -> Option<ActivationKind> {
    if matches!(rules, Ability::Loyalty { .. }) {
        return Some(ActivationKind::Loyalty);
    }
    match keyword_adding(state, address)? {
        Keyword::Equip(_) | Keyword::EquipTo { .. } => Some(ActivationKind::Equip),
        Keyword::Cycling(_) | Keyword::Typecycling { .. } => Some(ActivationKind::Cycling),
        _ => None,
    }
}

/// Whether an ability whose specific kind is `own` is of the kind `wanted`.
pub(crate) fn is_kind(wanted: ActivationKind, own: Option<ActivationKind>) -> bool {
    wanted == ActivationKind::Any || Some(wanted) == own
}

/// The mana of an activation with the cost modifiers in play applied: the increases first, then the reductions, and no
/// reduction takes the cost below the most `least` any applying reduction names.
pub(crate) fn modified(
    state: &GameState,
    source: ObjectId,
    address: AbilityRef,
    rules: &Ability,
    mana: ManaCost,
) -> ManaCost {
    let Some(target) = state
        .object(source)
        .map(crate::object::GameObject::reference)
    else {
        return mana;
    };
    let own = specific(state, address, rules);
    let (mut more, mut less, mut floor) = (ManaCost::ZERO, ManaCost::ZERO, 0u32);
    for id in state.zone(Zone::Battlefield).iter().copied() {
        let Some(holder) = state.object(id) else {
            continue;
        };
        if !definition_has_cost_modifier(holder.card()) {
            continue;
        }
        let controller = controller_of_object(state, holder);
        if !state.is_alive(controller) {
            continue;
        }
        for (_, ability) in abilities_of(state, id) {
            let Ability::CostModifier {
                applies_to,
                change,
                mana: unit,
                per,
                condition,
                activating: Some(kind),
                least,
            } = &*ability
            else {
                continue;
            };
            if !is_kind(*kind, own) {
                continue;
            }
            let scope = Scope::announcing(state, controller, holder.reference());
            if !scope.selects(applies_to, target).unwrap_or(false)
                || condition
                    .as_ref()
                    .is_some_and(|held| !scope.holds(held).unwrap_or(false))
            {
                continue;
            }
            let times = scope.amount(per).unwrap_or(0).clamp(0, MOST_TIMES);
            let part = match change {
                CostChange::Increase => &mut more,
                CostChange::Reduce => {
                    floor = floor.max(*least);
                    &mut less
                }
            };
            for _ in 0..times {
                *part = unit
                    .rest()
                    .symbols()
                    .iter()
                    .fold(part.clone(), |cost, symbol| cost.with(*symbol));
            }
        }
    }
    let raised = more
        .symbols()
        .iter()
        .fold(mana, |cost, symbol| cost.with(*symbol));
    let cut = crate::announce::c11a_reduced(&raised, &less);
    crate::announce::floored(&raised, cut, floor)
}
