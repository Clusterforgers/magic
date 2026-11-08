//! Assist (CR 702.132): as a spell with assist is cast, its caster may choose another player, who may pay any amount of the
//! generic mana of its total cost.
//!
//! The step comes after the total cost is known. The caster is asked which player (or no one) and that player how much;
//! the amount comes off the generic mana the caster pays. The helper then pays it with a payment decision of their own,
//! choosing their sources; its events wait in the announcement and are spent with the caster's payment, in the same
//! batch, so nothing is paid while the cast can still be backed out of. The helper's questions cannot be backed out of
//! and default to paying nothing and to the engine's own payment, so a seat that does not answer holds the cast up only
//! for its clock. The mana counts as spent to cast the spell (CR 601.2h).

use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaSymbol};

use super::modifiers::reduced;
use super::{Ctx, Next, What, Work};
use crate::choose::{ChoiceOption, NumberOptions, OptionOptions};
use crate::decision::{Decision, DecisionKind};
use crate::help::Offered;
use crate::mana::can_afford;
use crate::priority::mana_options;
use crate::spend::Spend;
use crate::state::GameState;

/// Whether the card `spell` has assist.
fn has_assist(state: &GameState, spell: ObjectId) -> bool {
    crate::keywords::keywords_of(state, spell)
        .into_iter()
        .any(|keyword| matches!(keyword, Keyword::Assist))
}

/// `k` generic mana.
fn generic(k: u32) -> ManaCost {
    ManaCost::from_symbols([ManaSymbol::Generic(k)])
}

/// How much generic mana of the spell `helper` can pay at most, up to `most`.
pub(crate) fn capacity(state: &GameState, helper: PlayerId, spell: ObjectId, most: u32) -> u32 {
    let Some(seat) = state.player(helper) else {
        return 0;
    };
    let sources = mana_options(state, helper);
    let spend = Spend::for_spell(state, spell, helper, false);
    (1..=most)
        .rev()
        .find(|k| can_afford(&generic(*k), seat.mana(), &sources, 0, &spend))
        .unwrap_or(0)
}

/// `cost` less the generic mana the best helper could pay, for a spell with assist; `None` when no one could help.
pub(crate) fn helped(
    state: &GameState,
    caster: PlayerId,
    spell: ObjectId,
    cost: &ManaCost,
) -> Option<ManaCost> {
    if cost.generic() == 0 || !has_assist(state, spell) {
        return None;
    }
    let best = state
        .alive_players()
        .filter(|id| *id != caster)
        .map(|id| capacity(state, id, spell, cost.generic()))
        .max()?;
    (best > 0).then(|| reduced(cost, &generic(best)))
}

/// The players who may be asked for help with the spell, in seat order, with how much each can pay.
fn helpers(
    state: &GameState,
    caster: PlayerId,
    spell: ObjectId,
    most: u32,
) -> Vec<(PlayerId, u32)> {
    state
        .alive_players()
        .filter(|id| *id != caster)
        .map(|id| (id, capacity(state, id, spell, most)))
        .filter(|(_, can)| *can > 0)
        .collect()
}

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    work.done.assist_paid = None;
    let What::Spell(spell) = ctx.what else {
        return None;
    };
    if !has_assist(ctx.state, *spell) {
        return None;
    }
    let total = work.total.as_mut()?;
    let cost = total.mana.as_ref()?;
    let most = cost.generic();
    let helpers = helpers(ctx.state, ctx.player, *spell, most);
    if most == 0 || helpers.is_empty() {
        return None;
    }
    let Some(pick) = work.done.assist_pick else {
        let mut options = vec![ChoiceOption::Label("No one".to_owned())];
        options.extend(helpers.iter().map(|(id, _)| ChoiceOption::Player(*id)));
        return Some(ctx.ask(DecisionKind::ChooseOption(OptionOptions {
            source: ctx.source,
            options,
            text: "Choose a player to help pay for it (assist).".to_owned(),
        })));
    };
    let (helper, can) = pick
        .checked_sub(1)
        .and_then(|at| helpers.get(at).copied())?;
    let Some(amount) = work.done.assist_amount else {
        return Some(Next::Ask(Decision {
            player: helper,
            kind: DecisionKind::ChooseNumber(NumberOptions {
                source: ctx.source,
                min: 0,
                max: Some(can),
                except: Vec::new(),
                text: "Pay any amount of the generic mana of this spell (assist).".to_owned(),
                can_cancel: false,
                cost: None,
                per_x: 0,
            }),
        }));
    };
    let amount = amount.min(can);
    if amount > 0 {
        // The helper chooses which of their sources pay (CR 702.132a); the question has the normal clock, and its
        // default is the engine's own payment.
        if work.done.assist_events.is_none() {
            let options = payment(ctx.state, *spell, helper, amount)?;
            return Some(Next::Ask(Decision {
                player: helper,
                kind: DecisionKind::PayMana(options),
            }));
        }
        total.mana = Some(reduced(cost, &generic(amount)));
        work.done.assist_paid = Some((helper, amount));
    }
    None
}

/// The helper's payment decision for `amount` generic mana of the spell; it cannot be taken back.
fn payment(
    state: &GameState,
    spell: ObjectId,
    helper: PlayerId,
    amount: u32,
) -> Option<crate::decision::PayOptions> {
    crate::cast::payment_decision(
        state,
        helper,
        spell,
        generic(amount),
        mana_options(state, helper),
        Offered::default(),
        Spend::for_spell(state, spell, helper, false),
        false,
        0,
    )
}
