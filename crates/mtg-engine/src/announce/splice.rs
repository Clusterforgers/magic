//! The splice step (CR 601.2b, 702.47a): the caster may reveal cards of their hand with splice onto the spell's kind.
//!
//! The cards that may be spliced are read when the announcement begins (`Needs::splice`); those whose cost the player can
//! pay with the rest of the total cost are offered in `ChooseCosts`, by position among them. The cards chosen are kept in
//! `Choices::spliced`: from then on the spell ability the announcement reads has their effects and targets
//! (`keywords::splice::with_spliced`), and their mana joins the total cost.

use mtg_core::mana::ManaCost;

use super::{Ctx, Next, What, Work, total};
use crate::choose::{Choices, CostsOptions, OptionalCost};
use crate::decision::DecisionKind;
use crate::keywords::splice::{Spliceable, spliceable};
use crate::state::GameState;

/// The cards `what` may have spliced onto it: none for anything but a spell.
pub(super) fn read(
    state: &GameState,
    player: mtg_core::ids::PlayerId,
    what: &What,
) -> Vec<Spliceable> {
    match what {
        What::Spell(spell) => spliceable(state, player, *spell),
        _ => Vec::new(),
    }
}

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    if work.done.splice {
        return None;
    }
    let offered: Vec<OptionalCost> = ctx
        .needs
        .splice
        .iter()
        .enumerate()
        .filter(|(_, card)| {
            let mut trial = work.choices.clone();
            trial.spliced.push(card.card);
            total::of(ctx, &trial).is_some_and(|total| total::affordable(ctx, &total))
        })
        .map(|(index, card)| OptionalCost {
            index,
            label: card.label.clone(),
        })
        .collect();
    if offered.is_empty() {
        work.done.splice = true;
        return None;
    }
    Some(ctx.ask(DecisionKind::ChooseCosts(CostsOptions {
        source: ctx.source,
        optional: offered,
        can_cancel: ctx.can_cancel(),
    })))
}

/// The cards an answer names, by position among those that may be spliced.
pub(super) fn chosen(splice: &[Spliceable], picked: &[usize]) -> Vec<mtg_core::ids::ObjectId> {
    picked
        .iter()
        .filter_map(|index| splice.get(*index).map(|card| card.card))
        .collect()
}

/// `mana` with the splice costs of the cards `choices` splices added.
pub(super) fn add(ctx: &Ctx<'_>, choices: &Choices, mana: Option<ManaCost>) -> Option<ManaCost> {
    let paid: Vec<&ManaCost> = choices
        .spliced
        .iter()
        .filter_map(|card| ctx.needs.splice.iter().find(|s| s.card == *card))
        .map(|card| &card.mana)
        .collect();
    if paid.is_empty() {
        return mana;
    }
    let symbols = mana
        .iter()
        .chain(paid)
        .flat_map(|cost| cost.symbols().iter().copied());
    Some(ManaCost::from_symbols(symbols))
}
