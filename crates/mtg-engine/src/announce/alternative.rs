//! The way a spell is cast (CR 601.2b, 118.9): for its mana cost, or for an alternative cost (flashback, evoke, overload,
//! madness, casting without paying its mana cost).
//!
//! `ways::of` lists the ways the card has where it is, with the cost each one pays in place of the mana cost. When more
//! than one can be used now (the targets can be announced and the cost paid) the player picks one with `ChooseOption`;
//! with one there is nothing to ask. The way chosen is kept in `Choices.way`, its cost replaces the mana cost in the total
//! cost, and the spell and the permanent it becomes remember it.

use mtg_core::mana::ManaCost;

use super::{Ctx, Granted, Next, What, Work, total};
use crate::choose::{ChoiceOption, Choices, OptionOptions};
use crate::decision::DecisionKind;
use crate::state::GameState;
use crate::ways::{self, Way};

/// The ways the card a spell is cast from has, in the order of the rules.
pub(super) fn read(
    state: &GameState,
    player: mtg_core::ids::PlayerId,
    what: &What,
    granted: Option<Granted>,
) -> Vec<Way> {
    match (what, granted) {
        (What::Spell(spell), Some(granted)) => {
            ways::granted(state, *spell, granted.free, granted.way)
        }
        (What::Spell(spell), None) => ways::of(state, player, *spell),
        (What::Ability(_) | What::Trigger(_) | What::Attack(..), _) => Vec::new(),
    }
}

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    if work.done.alternative {
        return None;
    }
    // A spell the rules give no way to cast is paid for with its mana cost.
    if ctx.needs.ways.is_empty() {
        // A resolution that lets the card be cast names how, and a card it cannot be cast in any way is not cast.
        if ctx.granted.is_some() {
            return Some(Next::Drop);
        }
        work.done.alternative = true;
        return None;
    }
    let offered = usable(ctx, work.choices);
    let picked = match offered.as_slice() {
        [] => return Some(Next::Drop),
        [only] => *only,
        _ => match work.done.way_picked {
            Some(pick) if pick < offered.len() => offered[pick],
            _ => {
                let options = offered
                    .iter()
                    .map(|index| ChoiceOption::Label(ctx.needs.ways[*index].label.clone()))
                    .collect();
                return Some(ctx.ask(DecisionKind::ChooseOption(OptionOptions {
                    source: ctx.source,
                    options,
                    text: "Choose how to cast it.".to_owned(),
                })));
            }
        },
    };
    work.choices.way = ctx.needs.ways[picked].alternative;
    work.choices.choose_cost(ctx.needs.ways[picked].variant);
    work.done.alternative = true;
    None
}

/// The ways, by index, that can be used now: the targets can be announced and the cost of the way can be paid.
fn usable(ctx: &Ctx<'_>, choices: &Choices) -> Vec<usize> {
    ctx.needs
        .ways
        .iter()
        .enumerate()
        .filter(|(_, way)| {
            let mut trial = choices.clone();
            trial.way = way.alternative;
            trial.choose_cost(way.variant);
            ways::announceable_in(ctx.state, ctx.player, ctx.source, way.alternative)
                && total::of(ctx, &trial).is_some_and(|total| total::affordable(ctx, &total))
        })
        .map(|(index, _)| index)
        .collect()
}

/// The mana cost the way chosen puts in place of `mana`, if the spell is cast for an alternative cost.
pub(super) fn replace(
    ctx: &Ctx<'_>,
    choices: &Choices,
    mana: Option<ManaCost>,
) -> Option<ManaCost> {
    match ctx.needs.ways.iter().find(|way| way.is(choices)) {
        Some(way) => way.cost(mana.as_ref()),
        None => mana,
    }
}
