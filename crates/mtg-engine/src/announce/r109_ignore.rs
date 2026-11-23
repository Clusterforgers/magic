//! Paying to ignore the effects of a source (CR 116.2, `GameRule::PayToIgnoreBy`): a special action that does not use
//! the stack. The announcement picks what the parts of the price take ("Sacrifice a permanent", "Exile three cards from
//! your graveyard"), one part at a time as for an ability's cost, then pays the mana, and can be backed out of until it
//! is paid.

use mtg_core::ids::{ObjectId, PlayerId};

use super::{Done, Next, Phase, suspend};
use crate::choose::{CardsOptions, Choices};
use crate::decision::{Decision, DecisionKind};
use crate::parts::Part;
use crate::state::GameState;

/// The next decision of ignoring the effects of `source` (a pick for a part of the price, then the payment), or
/// nothing when `player` cannot now.
pub(super) fn next(
    state: &GameState,
    player: PlayerId,
    source: ObjectId,
    choices: &mut Choices,
    done: &mut Done,
) -> Next {
    let Some(price) = crate::r6703_ignore::offer(state, player, source) else {
        return Next::Drop;
    };
    let mut settled = 0;
    for part in &price.parts {
        let Part::Pick(pick) = part else {
            continue;
        };
        if settled < choices.cost_picks().len() {
            settled += 1;
            continue;
        }
        let barred: Vec<ObjectId> = choices.cost_picks().iter().flatten().copied().collect();
        let mut owes = crate::r6703_ignore::payer(state, player, source);
        owes.barred = &barred;
        let candidates = pick.candidates(&owes);
        if pick.short(state, &candidates) {
            return Next::Drop;
        }
        if let Some(taken) = pick.unasked(state, &candidates) {
            choices.pick_for_cost(taken);
            settled += 1;
            continue;
        }
        let (min, max) = pick.bounds(candidates.len());
        done.asked = Some(Phase::Parts);
        return Next::Ask(Decision {
            player,
            kind: DecisionKind::ChooseCards(CardsOptions {
                one_per_name: false,
                source,
                from: candidates,
                min,
                max,
                text: pick.text(),
                can_cancel: true,
                total: None,
                groups: Vec::new(),
                targeted: Box::default(),
            }),
        });
    }
    let owes = crate::r6703_ignore::payer(state, player, source);
    if price
        .parts
        .iter()
        .filter(|part| !matches!(part, Part::Pick(_)))
        .any(|part| part.payable(&owes, &[]).is_none())
    {
        return Next::Drop;
    }
    let paid = choices.clone();
    suspend::pay(state, player, source, price.mana, || {
        crate::r6703_ignore::events(state, player, source, &paid)
    })
}
