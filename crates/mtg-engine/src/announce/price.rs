//! The costs a choice adds in the announcement (CR 601.2b, 601.2f): the cost of each mode of a modal spell (Spree,
//! Tiered) and the cost of each mode chosen beyond the first (Escalate).
//!
//! A mode is offered only when the spell can still be paid for with its cost on top of the rest of the total cost, and
//! the most modes offered are those the cheapest of them pay for, so a choice the player cannot pay is never offered. The
//! answer is checked again as a whole (`Pending::check_modes`), and the castable list judges a spell with the cheapest
//! modes it must choose (`floor`).

use std::borrow::Cow;

use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use super::{Ctx, optional, total};
use crate::choose::{Choices, ModeOptions, modal_of, mode_options};
use crate::parts::Part;
use crate::price::Price;
use crate::resolve::spell_ability;
use crate::state::GameState;

/// One cost a choice adds: the mana and the other parts it pays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Paid {
    /// The cost in the words of a card, for the prompt.
    pub(crate) label: String,
    /// The mana it adds to the total cost.
    pub(crate) mana: ManaCost,
    /// The other parts it pays: life and the objects the player picks.
    pub(crate) parts: Vec<Part>,
}

/// The costs of the modes of a modal spell, read from its rules when the announcement begins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModeCosts {
    /// The cost of each mode; `None` for one the engine does not pay, which is never offered.
    each: Vec<Option<Paid>>,
    /// The cost of each mode beyond the first.
    extra: Option<Paid>,
    /// The extra cost has a part the engine does not pay, so only one mode is chosen.
    one_only: bool,
}

/// What the engine pays of `cost` when a choice adds it, or `None` for a cost with a part it does not pay.
fn read(cost: &Cost) -> Option<Paid> {
    let price = Price::announced(cost).ok()?;
    if !optional::payable(&price) {
        return None;
    }
    let mana = price.mana.clone().unwrap_or_default();
    let mut words: Vec<String> = Vec::new();
    if price.mana.is_some() {
        words.push(mana.to_string());
    }
    words.extend(price.parts.iter().map(Part::text));
    Some(Paid {
        label: words.join(", "),
        mana,
        parts: price.parts,
    })
}

impl ModeCosts {
    /// The costs of the modes of `rules`, or `None` for a spell whose modes cost nothing.
    pub(super) fn of(rules: Option<&Ability>) -> Option<ModeCosts> {
        let modal = modal_of(rules?)?;
        if modal.mode_costs.is_empty() && modal.extra_mode_cost.is_none() {
            return None;
        }
        let each = (0..modal.modes.len())
            .map(|index| match modal.mode_costs.get(index) {
                Some(cost) => read(cost),
                None => Some(Paid {
                    label: String::new(),
                    mana: ManaCost::ZERO,
                    parts: Vec::new(),
                }),
            })
            .collect();
        let printed = modal.extra_mode_cost;
        Some(ModeCosts {
            each,
            extra: printed.and_then(read),
            one_only: printed.is_some_and(|cost| read(cost).is_none()),
        })
    }

    /// The cost of mode `index`, as the prompt words it; nothing for a mode without one.
    pub(crate) fn label(&self, index: usize) -> Option<&str> {
        self.each
            .get(index)?
            .as_ref()
            .map(|paid| paid.label.as_str())
            .filter(|label| !label.is_empty())
    }

    /// The costs the modes `chosen` add: each mode's own, and the extra cost once for each mode after the first.
    fn charged(&self, chosen: &[usize]) -> Vec<&Paid> {
        let mut paid: Vec<&Paid> = chosen
            .iter()
            .filter_map(|index| self.each.get(*index)?.as_ref())
            .collect();
        if let Some(extra) = &self.extra {
            paid.extend(std::iter::repeat_n(extra, chosen.len().saturating_sub(1)));
        }
        paid
    }

    /// The parts the modes `chosen` pay besides mana.
    pub(super) fn parts(&self, chosen: &[usize]) -> Vec<Part> {
        self.charged(chosen)
            .into_iter()
            .flat_map(|paid| paid.parts.iter().cloned())
            .collect()
    }

    /// Whether the engine pays the cost of mode `index`.
    fn payable(&self, index: usize) -> bool {
        self.each.get(index).is_some_and(Option::is_some)
    }

    /// The most modes the extra cost lets be chosen: one when the engine does not pay it.
    fn at_most(&self) -> usize {
        if self.one_only { 1 } else { usize::MAX }
    }
}

/// `mana` with the mana of the modes chosen added.
pub(super) fn add(ctx: &Ctx<'_>, choices: &Choices, mana: Option<ManaCost>) -> Option<ManaCost> {
    let Some(costs) = &ctx.needs.modes else {
        return mana;
    };
    let paid = costs.charged(&choices.modes);
    if paid.is_empty() {
        return mana;
    }
    let symbols = mana
        .iter()
        .chain(paid.iter().map(|paid| &paid.mana))
        .flat_map(|cost| cost.symbols().iter().copied());
    Some(ManaCost::from_symbols(symbols))
}

/// Whether the spell can be paid for with only the modes `modes` chosen.
fn fits(ctx: &Ctx<'_>, choices: &Choices, modes: &[usize]) -> bool {
    let mut trial = choices.clone();
    trial.modes = modes.to_vec();
    trial.modes.sort_unstable();
    total::of(ctx, &trial).is_some_and(|total| total::affordable(ctx, &total))
}

/// Narrows the modes offered to those the player can pay for, and the most to choose to what the cheapest pay for.
/// Returns whether enough remain to choose the fewest.
pub(super) fn restrict(ctx: &Ctx<'_>, choices: &Choices, options: &mut ModeOptions) -> bool {
    let Some(costs) = &ctx.needs.modes else {
        return true;
    };
    options
        .modes
        .retain(|mode| costs.payable(mode.index) && fits(ctx, choices, &[mode.index]));
    for mode in &mut options.modes {
        mode.cost = costs.label(mode.index).map(str::to_owned);
    }
    let mut order: Vec<usize> = options.modes.iter().map(|mode| mode.index).collect();
    order.sort_by_key(|index| {
        let paid = costs.charged(&[*index]);
        (
            paid.first().map_or(0, |paid| paid.mana.mana_value()),
            *index,
        )
    });
    let mut most = 0;
    while most < order.len().min(costs.at_most()) && fits(ctx, choices, &order[..=most]) {
        most += 1;
    }
    options.cheapest = order[..most].to_vec();
    let most = u32::try_from(most).unwrap_or(u32::MAX);
    options.max = options.max.min(most);
    options.min <= options.max
}

/// Whether the spell can be paid for with the modes `modes` chosen, which an answer is checked against.
pub(super) fn affordable_set(ctx: &Ctx<'_>, choices: &Choices, modes: &[usize]) -> bool {
    ctx.needs.modes.as_ref().is_none_or(|costs| {
        modes.iter().all(|mode| costs.payable(*mode))
            && modes.len() <= costs.at_most()
            && fits(ctx, choices, modes)
    })
}

/// `cost` with the mana the cheapest modes add that the spell must choose, which the castable list judges against the
/// mana the player has; `None` when fewer modes than the fewest to choose can be paid for at all.
pub(crate) fn floor<'a>(
    state: &GameState,
    player: PlayerId,
    spell: ObjectId,
    cost: &'a ManaCost,
) -> Option<Cow<'a, ManaCost>> {
    let Some((_, rules)) = spell_ability(state, spell) else {
        return Some(Cow::Borrowed(cost));
    };
    let Some(costs) = ModeCosts::of(Some(&rules)) else {
        return Some(Cow::Borrowed(cost));
    };
    let Some(modal) = modal_of(&rules) else {
        return Some(Cow::Borrowed(cost));
    };
    let Some(options) = mode_options(state, spell, player, &modal, true, None) else {
        return Some(Cow::Borrowed(cost));
    };
    let mut cheapest: Vec<usize> = options
        .modes
        .iter()
        .map(|mode| mode.index)
        .filter(|index| costs.payable(*index))
        .collect();
    cheapest.sort_by_key(|index| {
        let paid = costs.charged(&[*index]);
        (
            paid.first().map_or(0, |paid| paid.mana.mana_value()),
            *index,
        )
    });
    if cheapest.len() < usize::try_from(options.min).unwrap_or(usize::MAX) {
        return None;
    }
    cheapest.truncate(usize::try_from(options.min).unwrap_or(usize::MAX));
    let paid = costs.charged(&cheapest);
    if paid.is_empty() {
        return Some(Cow::Borrowed(cost));
    }
    let symbols = cost.symbols().iter().copied().chain(
        paid.iter()
            .flat_map(|paid| paid.mana.symbols().iter().copied()),
    );
    Some(Cow::Owned(ManaCost::from_symbols(symbols)))
}
