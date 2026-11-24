//! "X is the number of artifact and/or creature cards in an opponent's graveyard as you cast this spell"
//! (`CostItem::R121XIsPerOpponent`, CR 107.3c, 601.2b): the caster picks the opponent as X is fixed, so the X offered is
//! one of the counts of the opponents' graveyards, and no other value.

use mtg_cards::ability::Ability;
use mtg_cards::cost::CostItem;
use mtg_cards::refs::{Pred, Selector, ZoneKind};

use super::{Ctx, What};
use crate::resolve::Scope;
use crate::zones::Zone;

/// The values X may take: for each opponent of the caster still in the game, the number of cards in their graveyard
/// the spell's selector names, sorted and each once; `None` for anything without such a cost.
pub(super) fn r121_x_per_opponent(ctx: &Ctx<'_>) -> Option<Vec<u32>> {
    let What::Spell(spell) = ctx.what else {
        return None;
    };
    let object = ctx.state.object(*spell)?;
    let filter =
        crate::ability_reader::printed_rules(object.card()).find_map(|rule| match &**rule {
            Ability::AdditionalCost {
                cost,
                required: true,
                ..
            } => cost.0.iter().find_map(|item| match item {
                CostItem::R121XIsPerOpponent(filter) => Some(filter.clone()),
                _ => None,
            }),
            _ => None,
        })?;
    let scope = Scope::announcing(ctx.state, ctx.player, object.reference());
    let mut selector = filter.0;
    selector.push(Pred::In(ZoneKind::Graveyard));
    let found = scope.matching(&Selector(selector)).ok()?;
    let mut counts: Vec<u32> = ctx
        .state
        .turn_order_from(ctx.player)
        .into_iter()
        .filter(|player| ctx.state.is_alive(*player) && ctx.state.is_opponent(ctx.player, *player))
        .map(|player| {
            let held = found
                .iter()
                .filter(|named| {
                    ctx.state
                        .object(named.id)
                        .is_some_and(|card| card.zone() == Zone::Graveyard(player))
                })
                .count();
            u32::try_from(held).unwrap_or(u32::MAX)
        })
        .collect();
    counts.sort_unstable();
    counts.dedup();
    Some(counts)
}

/// Excepts from the values `min..=max` offered for X every value that is no opponent's count; false when none of them
/// is one, so the spell cannot be cast. Any X is kept for a spell without such a cost.
pub(super) fn r121_except_others(
    ctx: &Ctx<'_>,
    (min, max): (u32, u32),
    except: &mut Vec<u32>,
) -> bool {
    let Some(allowed) = r121_x_per_opponent(ctx) else {
        return true;
    };
    if !(min..=max).any(|x| allowed.contains(&x)) {
        return false;
    }
    except.extend((min..=max).filter(|x| !allowed.contains(x)));
    true
}
