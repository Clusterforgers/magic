//! The most X a spell may announce under "X can't be greater than ..." (`CostItem::XAtMost` in a required
//! `AdditionalCost`, Open the Way), read as the spell is announced.

use mtg_cards::ability::Ability;
use mtg_cards::cost::CostItem;

use super::{Ctx, What};
use crate::resolve::Scope;

/// The least of the caps the spell's required additional costs set on X, never below 0; `None` without one or for
/// anything but a spell.
pub(super) fn r5523_spell_cap(ctx: &Ctx<'_>) -> Option<u32> {
    let What::Spell(spell) = ctx.what else {
        return None;
    };
    let object = ctx.state.object(*spell)?;
    let scope = Scope::announcing(ctx.state, ctx.player, object.reference());
    crate::ability_reader::printed_rules(object.card())
        .filter_map(|rule| match &**rule {
            Ability::AdditionalCost {
                cost,
                required: true,
                ..
            } => Some(cost),
            _ => None,
        })
        .flat_map(|cost| cost.0.iter())
        .filter_map(|item| match item {
            CostItem::XAtMost(most) | CostItem::XIs(most) => scope.amount(most).ok(),
            _ => None,
        })
        .map(|most| u32::try_from(most.max(0)).unwrap_or(u32::MAX))
        .min()
}

/// The X the text of the spell or of the activated ability defines (`CostItem::XIs`, CR 107.3c), counted as it is
/// announced; `None` when no player-free X is named.
pub(super) fn r6503_fixed_x(ctx: &Ctx<'_>) -> Option<u32> {
    let object = ctx.state.object(ctx.source)?;
    let scope = Scope::announcing(ctx.state, ctx.player, object.reference());
    let fixed = |cost: &mtg_cards::cost::Cost| {
        cost.0.iter().find_map(|item| match item {
            CostItem::XIs(amount) => scope.amount(amount).ok(),
            _ => None,
        })
    };
    let value = match (ctx.what, ctx.ability) {
        (What::Spell(_), _) => {
            crate::ability_reader::printed_rules(object.card()).find_map(|rule| match &**rule {
                Ability::AdditionalCost {
                    cost,
                    required: true,
                    ..
                } => fixed(cost),
                _ => None,
            })
        }
        (What::Ability(_), Some(Ability::Activated { cost, .. })) => fixed(cost),
        _ => None,
    }?;
    Some(u32::try_from(value.max(0)).unwrap_or(u32::MAX))
}

/// The amount an activated ability's text defines its X as when it reads the ability's targets ("X is twice the mana
/// value of that spell", `CostItem::XIs`): the targets are chosen after X would be announced (CR 601.2b, 601.2c), so the
/// X is worked out with the total cost (CR 601.2f) and never asked.
pub(super) fn r116_x_of_targets<'a>(ctx: &Ctx<'a>) -> Option<&'a mtg_cards::value::Amount> {
    let cost = match (ctx.what, ctx.ability) {
        (What::Spell(_), _) => return r117_spell_x_of_targets(ctx),
        (_, Some(Ability::Activated { cost, .. })) => cost,
        _ => return None,
    };
    cost.0.iter().find_map(|item| match item {
        CostItem::XIs(amount) if r116_names_target(&serde_json::to_value(amount).ok()?) => {
            Some(amount)
        }
        _ => None,
    })
}

/// The X a spell's required additional cost defines from the spell's targets ("collect evidence X, where X is the total
/// mana value of the permanents this spell targets", CR 601.2b, 601.2f): the parts that take X are picked once the
/// targets are chosen (`parts.rs`).
fn r117_spell_x_of_targets<'a>(ctx: &Ctx<'a>) -> Option<&'a mtg_cards::value::Amount> {
    let What::Spell(spell) = ctx.what else {
        return None;
    };
    let object = ctx.state.object(*spell)?;
    crate::ability_reader::printed_rules(object.card()).find_map(|rule| match &**rule {
        Ability::AdditionalCost {
            cost,
            required: true,
            ..
        } => cost.0.iter().find_map(|item| match item {
            CostItem::XIs(amount) if r116_names_target(&serde_json::to_value(amount).ok()?) => {
                Some(amount)
            }
            _ => None,
        }),
        _ => None,
    })
}

/// Whether a value of the data names a target (`Ref::Target`).
fn r116_names_target(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(fields) => {
            fields.contains_key("Target") || fields.values().any(r116_names_target)
        }
        serde_json::Value::Array(items) => items.iter().any(r116_names_target),
        _ => false,
    }
}

/// The X of `r116_x_of_targets` counted with the targets announced in `choices`, never below 0.
pub(super) fn r116_x_with_targets(ctx: &Ctx<'_>, choices: &crate::choose::Choices) -> Option<u32> {
    let amount = r116_x_of_targets(ctx)?;
    let object = ctx.state.object(ctx.source)?;
    let scope = Scope {
        choices,
        ..Scope::announcing(ctx.state, ctx.player, object.reference())
    };
    let value = scope.amount(amount).ok()?;
    Some(u32::try_from(value.max(0)).unwrap_or(u32::MAX))
}
