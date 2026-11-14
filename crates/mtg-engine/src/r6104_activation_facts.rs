//! What the payment of an activated ability on the stack did: whether its cost sacrificed a permanent and how much life
//! it paid (CR 602.2b, 601.2h).

use mtg_cards::ability::Ability;
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::value::Amount;

use crate::object::GameObject;

/// The cost of the activated ability on the stack, with the X it was activated for.
fn r6104_activated_cost(ability: &GameObject) -> Option<(&Cost, u32)> {
    let Ability::Activated { cost, .. } = ability.ability()?.rules() else {
        return None;
    };
    Some((cost, ability.choices().x.unwrap_or(0)))
}

/// Whether the cost of the activated ability on the stack sacrificed a permanent: the source, or one or more others. A
/// mana ability activated to pay it is not part of its cost.
pub(crate) fn r6104_paid_by_sacrifice(ability: &GameObject) -> bool {
    let Some((cost, x)) = r6104_activated_cost(ability) else {
        return false;
    };
    cost.0.iter().any(|item| match item {
        CostItem::SacrificeSelf => true,
        CostItem::Sacrifice { count, .. } => match count {
            Amount::Fixed(n) => *n > 0,
            Amount::X => x > 0,
            _ => !ability.choices().cost_shots().is_empty(),
        },
        _ => false,
    })
}

/// The life paid to activate the activated ability on the stack: the life its cost names and the life paid for its
/// Phyrexian mana symbols; none for a life cost the engine cannot count.
pub(crate) fn r6104_life_paid(ability: &GameObject) -> Option<i64> {
    let (cost, x) = r6104_activated_cost(ability)?;
    let mut life = i64::from(
        ability
            .choices()
            .cost_taken
            .as_deref()
            .map_or(0, |taken| taken.phyrexian_life),
    );
    for item in &cost.0 {
        if let CostItem::PayLife(amount) = item {
            life += match amount {
                Amount::Fixed(n) => i64::from(*n),
                Amount::X => i64::from(x),
                _ => return None,
            };
        }
    }
    Some(life)
}
