//! "For one spell this turn, you may spend mana as though it were mana of any type to pay that spell's mana cost."
//! (`SpellBenefit::R112SpendAs` on a `NextSpell` rule, CR 609.4b): the mana cost of the next spell the rule's
//! controller casts, and only its mana cost, is paid by mana of any color or type; the cast ends the rule.

use mtg_cards::modify::{SpellBenefit, SpendAs};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use crate::resolve::Scope;
use crate::rules::Rule;
use crate::state::GameState;

/// How the mana cost of `spell` that `player` casts may be paid under the `NextSpell` rules in force: any type over any
/// color over as printed.
pub(crate) fn r112_next_spend_as(state: &GameState, player: PlayerId, spell: ObjectId) -> SpendAs {
    let Some(target) = state
        .object(spell)
        .map(crate::object::GameObject::reference)
    else {
        return SpendAs::Normal;
    };
    let mut found = SpendAs::Normal;
    for effect in state
        .rule_effects
        .iter()
        .filter(|effect| effect.controller == player)
    {
        let Rule::NextSpell {
            what,
            benefit: SpellBenefit::R112SpendAs(spend_as),
        } = &effect.rule
        else {
            continue;
        };
        let scope = Scope {
            casting: Some((spell, player)),
            ..Scope::announcing(state, player, effect.source.unwrap_or(target))
        };
        if !what.is_empty() && !scope.selects(what, target).unwrap_or(false) {
            continue;
        }
        match spend_as {
            SpendAs::AnyType => return SpendAs::AnyType,
            SpendAs::AnyColor => found = SpendAs::AnyColor,
            SpendAs::Normal => {}
        }
    }
    found
}

/// The printed mana cost of `spell`, if it has one, as `player` may pay it under those rules.
pub(crate) fn r112_printed(
    state: &GameState,
    player: PlayerId,
    spell: ObjectId,
    printed: Option<ManaCost>,
) -> Option<ManaCost> {
    printed.map(|cost| r112_mana_cost(state, player, spell, cost))
}

/// The mana cost `cost` of `spell` as `player` may pay it under those rules.
pub(crate) fn r112_mana_cost(
    state: &GameState,
    player: PlayerId,
    spell: ObjectId,
    cost: ManaCost,
) -> ManaCost {
    match r112_next_spend_as(state, player, spell) {
        SpendAs::AnyType => cost.paid_by_any_mana(),
        SpendAs::AnyColor => cost.paid_by_any_color(),
        SpendAs::Normal => cost,
    }
}
