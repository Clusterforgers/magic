//! The kind of an activated ability (`ActivationKind`) and the cost modifiers that change the cost of activating abilities
//! of a kind (CR 601.2f, 602.2b): "Equip abilities you activate cost {1} less", "Activated abilities of creatures you
//! control cost {2} less to activate. This effect can't reduce the mana in that cost to less than one mana."

use mtg_cards::ability::{Ability, ActivationKind, CostChange};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::Selector;
use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;

use crate::abilities::AbilityRef;
use crate::ability_reader::{
    abilities_of, definition_has_cost_modifier, is_cost_modifier, keyword_adding, may_hold,
};
use crate::characteristics::controller_of_object;
use crate::object::ObjectRef;
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// The most times one modifier changes a cost: a count this large is more mana than any game has.
const MOST_TIMES: i64 = 100;

/// The one specific kind the ability `rules`, which `address` names, is: a loyalty ability, the equip, cycling or
/// unearth ability; none for any other activated ability.
pub(crate) fn specific(
    state: &GameState,
    address: AbilityRef,
    rules: &Ability,
) -> Option<ActivationKind> {
    of_keyword(rules, keyword_adding(state, address))
}

/// [`specific`] for an ability whose source has left since `shot` was taken.
pub(crate) fn specific_as_it_was(
    state: &GameState,
    address: AbilityRef,
    rules: &Ability,
    shot: &crate::snapshot::Snapshot,
) -> Option<ActivationKind> {
    of_keyword(
        rules,
        crate::ability_reader::keyword_adding_as_it_was(state, address, shot),
    )
}

/// The kind of `rules`, which `keyword` adds when a keyword adds it.
fn of_keyword(rules: &Ability, keyword: Option<Keyword>) -> Option<ActivationKind> {
    if matches!(rules, Ability::Loyalty { .. }) {
        return Some(ActivationKind::Loyalty);
    }
    match keyword? {
        Keyword::Equip(_) | Keyword::EquipTo { .. } => Some(ActivationKind::Equip),
        Keyword::Cycling(_) | Keyword::Typecycling { .. } => Some(ActivationKind::Cycling),
        Keyword::Unearth(_) => Some(ActivationKind::Unearth),
        Keyword::Ninjutsu(_) | Keyword::CommanderNinjutsu(_) => Some(ActivationKind::Ninjutsu),
        _ => None,
    }
}

/// Whether an ability whose specific kind is `own` is of the kind `wanted`.
pub(crate) fn is_kind(wanted: ActivationKind, own: Option<ActivationKind>) -> bool {
    // Unlocking and turning face up are no abilities: only a change of their own costs applies to them.
    let special =
        |kind: ActivationKind| matches!(kind, ActivationKind::Unlock | ActivationKind::Morph);
    if own.is_some_and(special) || special(wanted) {
        return own == Some(wanted);
    }
    match wanted {
        ActivationKind::Every => true,
        ActivationKind::Any => own != Some(ActivationKind::Mana),
        ActivationKind::AnyButLoyalty => {
            own != Some(ActivationKind::Mana) && own != Some(ActivationKind::Loyalty)
        }
        _ => Some(wanted) == own,
    }
}

#[cfg(test)]
thread_local! {
    /// How many times `modified_as` walked the battlefield on this thread: the work a test bounds.
    pub(crate) static WALKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The mana of an activation with the cost modifiers in play applied: the increases first, then the reductions, and no
/// reduction takes the cost below the most `least` any applying reduction names. `targeted` is what the activation targets
/// once announced; `None` before.
pub(crate) fn modified(
    state: &GameState,
    (source, address, rules): (ObjectId, AbilityRef, &Ability),
    mana: ManaCost,
    targeted: Option<&[ObjectRef]>,
) -> ManaCost {
    modified_as(
        state,
        source,
        specific(state, address, rules),
        mana,
        targeted,
    )
}

/// Whether an activation that targets `targeted` (`None` before its targets are announced) has a target `selector` selects.
/// Before the announcement the activation is judged at its cheapest, so it is offered whenever some target makes it
/// affordable: a reduction (`reduces`) applies while some permanent matches, an increase does not apply.
fn targets_one(
    scope: &Scope<'_>,
    selector: &Selector,
    targeted: Option<&[ObjectRef]>,
    reduces: bool,
) -> bool {
    let picks = |object: ObjectRef| scope.selects(selector, object).unwrap_or(false);
    match targeted {
        Some(targeted) => targeted.iter().copied().any(picks),
        None if !reduces => false,
        None => scope
            .state
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| scope.state.object(*id))
            .map(crate::object::GameObject::reference)
            .any(picks),
    }
}

/// The mana of a mana ability of `source` with the cost modifiers of mana abilities and of every activated ability
/// applied (CR 601.2f, 605.3a): "Enchanted creature's activated abilities cost {3} more to activate".
pub(crate) fn mana_modified(state: &GameState, source: ObjectId, mana: ManaCost) -> ManaCost {
    modified_as(state, source, Some(ActivationKind::Mana), mana, Some(&[]))
}

/// The mana of unlocking a door of the Room `room` with the changes of unlock costs in play applied ("Unlock costs you
/// pay cost {1} less").
pub(crate) fn r402_unlock_modified(state: &GameState, room: ObjectId, mana: ManaCost) -> ManaCost {
    modified_as(state, room, Some(ActivationKind::Unlock), mana, Some(&[]))
}

/// The mana of turning the face-down permanent `id` face up for its morph or megamorph cost with the changes of morph
/// costs in play applied ("All morph costs cost {2} more").
pub(crate) fn r402_morph_modified(state: &GameState, id: ObjectId, mana: ManaCost) -> ManaCost {
    modified_as(state, id, Some(ActivationKind::Morph), mana, Some(&[]))
}

/// The mana of an activation of an ability of the specific kind `own` with the cost modifiers in play applied.
fn modified_as(
    state: &GameState,
    source: ObjectId,
    own: Option<ActivationKind>,
    mana: ManaCost,
    targeted: Option<&[ObjectRef]>,
) -> ManaCost {
    // No cost modifier exists in the game, so the walk over the battlefield would find none.
    if !state.possible.costs {
        return mana;
    }
    #[cfg(test)]
    WALKS.with(|walks| walks.set(walks.get() + 1));
    let Some(target) = state
        .object(source)
        .map(crate::object::GameObject::reference)
    else {
        return mana;
    };
    let (mut more, mut less, mut floor) = (ManaCost::ZERO, ManaCost::ZERO, 0u32);
    for id in state.zone(Zone::Battlefield).iter().copied() {
        let Some(holder) = state.object(id) else {
            continue;
        };
        if !may_hold(
            state,
            holder,
            definition_has_cost_modifier,
            is_cost_modifier,
        ) {
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
                life: 0,
                targeting,
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
                || targeting.as_ref().is_some_and(|wanted| {
                    !targets_one(&scope, wanted, targeted, *change == CostChange::Reduce)
                })
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

#[cfg(test)]
mod r301_tests {
    //! Goal: the ability a keyword adds is of that keyword's specific kind (equip, cycling, unearth, ninjutsu), a loyalty ability is a loyalty ability whatever adds it, and any other keyword's ability has no specific kind.
    use mtg_cards::ability::{Ability, ActivationKind};
    use mtg_cards::cost::Cost;
    use mtg_cards::keyword::Keyword;
    use rstest::rstest;

    use super::of_keyword;

    fn activated() -> Ability {
        serde_json::from_str(r#"{"Activated":{"cost":"{1}","effects":[{"Draw":{}}]}}"#).unwrap()
    }

    #[rstest]
    #[case::equip(Keyword::Equip(Cost::default()), Some(ActivationKind::Equip))]
    #[case::cycling(Keyword::Cycling(Cost::default()), Some(ActivationKind::Cycling))]
    #[case::unearth(Keyword::Unearth(Cost::default()), Some(ActivationKind::Unearth))]
    #[case::r301_ninjutsu(Keyword::Ninjutsu(Cost::default()), Some(ActivationKind::Ninjutsu))]
    #[case::r5511_commander_ninjutsu(
        Keyword::CommanderNinjutsu(Cost::default()),
        Some(ActivationKind::Ninjutsu)
    )]
    #[case::scavenge_has_none(Keyword::Scavenge(Cost::default()), None)]
    fn the_kind_of_a_keyword_ability(
        #[case] keyword: Keyword,
        #[case] kind: Option<ActivationKind>,
    ) {
        assert_eq!(of_keyword(&activated(), Some(keyword)), kind);
        assert_eq!(of_keyword(&activated(), None), None);
    }
}
