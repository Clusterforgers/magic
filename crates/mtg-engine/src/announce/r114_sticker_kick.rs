//! Sticker kicker (`Rule::R114StickerKicker`, CR 123, the Wicker Picker rulings): for each sticker kicker the caster
//! pays, they get a ticket and may put a sticker on the spell just before it becomes cast. The sticker is chosen as the
//! spell is announced, with the parts of its costs, among the unused stickers whose tickets the caster will hold by
//! then; a spell its caster does not own gets the ticket and no sticker. The spell keeps the choices in
//! `CostTaken.r114_stickers`, and the cast logs the tickets and stickers before `SpellCast` (`cast::spell_events`).

use super::{Ctx, Next, What, Work};
use crate::choose::{ChoiceOption, OptionOptions, R114StickerKick};
use crate::decision::DecisionKind;
use crate::keywords::OptionalKind;
use crate::un34_stickers::{StickerAt, r114_unused, sticker_text, ticket};

/// The indices among the optional costs of the sticker kickers chosen, in the order chosen.
fn kicks(ctx: &Ctx<'_>, work: &Work<'_>) -> Vec<usize> {
    work.choices
        .optional_costs
        .iter()
        .copied()
        .filter(|index| {
            ctx.needs
                .optional
                .get(*index)
                .is_some_and(|cost| cost.kind == OptionalKind::R114StickerKicker)
        })
        .collect()
}

/// The stickers the caster may put on the spell for the next sticker kicker: unused, not chosen for an earlier one, and
/// paid for with the tickets held once this kicker's ticket is gained.
fn offered(ctx: &Ctx<'_>, work: &Work<'_>) -> Vec<StickerAt> {
    let state = ctx.state;
    let chosen: Vec<StickerAt> = work
        .choices
        .r114_stickers()
        .iter()
        .filter_map(|kick| kick.sticker)
        .collect();
    let held = state
        .player(ctx.player)
        .map_or(0, |seat| seat.counters_of(&ticket()));
    let gained = u32::try_from(work.choices.r114_stickers().len() + 1).unwrap_or(u32::MAX);
    let spent: u32 = chosen
        .iter()
        .filter_map(|at| state.sticker(*at))
        .map(mtg_cards::ability::Sticker::tickets)
        .sum();
    let tickets = (held + gained).saturating_sub(spent);
    r114_unused(state, ctx.player)
        .into_iter()
        .filter(|(at, sticker)| !chosen.contains(at) && sticker.tickets() <= tickets)
        .map(|(at, _)| at)
        .collect()
}

/// Settles the sticker of each sticker kicker paid, asking with `ChooseOption` (no sticker first); `None` once every one
/// is settled or the spell pays none.
pub(super) fn r114_run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    let What::Spell(spell) = ctx.what else {
        return None;
    };
    let kicks = kicks(ctx, work);
    let owned = ctx
        .state
        .object(*spell)
        .is_some_and(|object| object.owner() == ctx.player);
    while let Some(&cost) = kicks.get(work.choices.r114_stickers().len()) {
        let stickers = if owned {
            offered(ctx, work)
        } else {
            Vec::new()
        };
        if stickers.is_empty() {
            work.choices.r114_add_kick(R114StickerKick {
                cost,
                sticker: None,
            });
            continue;
        }
        // The answer is the one to this question: the picks of later questions of the parts start unset.
        work.done.w2f_kind_picked = None;
        if let Some(pick) = work.done.r114_picked.take() {
            let sticker = match pick {
                0 => None,
                at => Some(*stickers.get(at - 1)?),
            };
            work.choices
                .r114_add_kick(R114StickerKick { cost, sticker });
            continue;
        }
        let mut options = vec![ChoiceOption::Label("Put no sticker on it".to_owned())];
        options.extend(stickers.iter().filter_map(|at| {
            let sheet = ctx
                .state
                .sticker_sheets(ctx.player)
                .into_iter()
                .find(|(index, _)| *index == at.sheet)
                .map_or_else(String::new, |(_, card)| card.name.clone());
            let sticker = ctx.state.sticker(*at)?;
            Some(ChoiceOption::Label(format!(
                "{sheet}: {}",
                sticker_text(sticker)
            )))
        }));
        return Some(ctx.ask(DecisionKind::ChooseOption(OptionOptions {
            source: ctx.source,
            options,
            text: "Sticker kicker: choose a sticker to put on the spell.".to_owned(),
            can_cancel: ctx.can_cancel(),
        })));
    }
    None
}
