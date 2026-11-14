//! "... unless they sacrifice a creature or pay 3 life": a payment of one of several costs. Each player the effect names
//! is asked in turn order which of the costs they can pay they pay, or none (CR 118.3), until one pays; the payer then
//! picks what the chosen cost takes and pays its mana as for a single cost (`pay.rs`).

use mtg_cards::cost::Cost;
use mtg_cards::refs::Ref;
use mtg_core::ids::PlayerId;

use super::pay::{
    Settled, asked_order, c6_eg_counted, can_pay, price_of, said, settle, unexpected,
};
use crate::choose::Choice;
use crate::choose::{ChoiceOption, OptionOptions};
use crate::decision::{Decision, DecisionKind};
use crate::price::Price;
use crate::resolve::{Cx, Next, ResolveError, Run};

/// The words of the way out.
const NONE: &str = "Pay none of them";

/// Runs a payment of one of `alternatives` by the players `who` names. The journal holds, for each player asked, the
/// index of the option taken among those offered (the costs they can pay, in printed order, then none), then the picks
/// and payment of the cost taken.
pub(super) fn run(cx: &Cx<'_>, who: &Ref, alternatives: &[Cost]) -> Result<Run, ResolveError> {
    let prices = alternatives
        .iter()
        .map(|cost| {
            let price = price_of(&c6_eg_counted(cx, cost)?)?;
            if price.x > 0 {
                return Err(ResolveError::Unsupported("Cost.OneOf.Elsewhere"));
            }
            Ok(Price {
                granter: cx.scope.granter,
                ..price
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let state = cx.state();
    let mut journal = cx
        .journal()
        .iter()
        .filter(|entry| state.is_alive(entry.0))
        .peekable();
    let mut paid = None;
    for player in asked_order(cx.scope.players(who)?, cx.journal()) {
        let offered: Vec<usize> = (0..prices.len())
            .filter(|at| {
                prices
                    .get(*at)
                    .is_some_and(|price| can_pay(cx, player, price))
            })
            .collect();
        let Some((_, asked)) = journal.next_if(|entry| entry.0 == player) else {
            if state.is_alive(player) {
                return Ok(Run::Ask(question(cx, player, &prices, &offered)));
            }
            continue;
        };
        let Choice::Option(index) = asked else {
            return Err(unexpected(asked, player));
        };
        if *index == offered.len() {
            continue;
        }
        let Some(price) = offered.get(*index).and_then(|at| prices.get(*at)) else {
            return Err(ResolveError::Invalid(format!(
                "{player:?} took a cost that was not offered"
            )));
        };
        match settle(cx, player, price, &mut journal)? {
            Settled::Ask(decision) => return Ok(Run::Ask(decision)),
            Settled::Paid(events, _) => {
                paid = Some(events);
                break;
            }
        }
    }
    if journal.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a question that was not asked".into(),
        ));
    }
    Ok(Run::Done {
        next: Next::Enter(usize::from(paid.is_none())),
        events: paid.unwrap_or_default(),
        bound: Vec::new(),
        snapshots: Vec::new(),
    })
}

/// The costs `player` can pay, each in the words of the card ("Sacrifice a creature", "Pay 3 life"), then none.
fn question(cx: &Cx<'_>, player: PlayerId, prices: &[Price], offered: &[usize]) -> Decision {
    let options = offered
        .iter()
        .filter_map(|at| prices.get(*at))
        .map(|price| ChoiceOption::Label(said(price).join(", ")))
        .chain(std::iter::once(ChoiceOption::Label(NONE.to_owned())))
        .collect();
    Decision {
        player,
        kind: DecisionKind::ChooseOption(OptionOptions {
            source: cx.scope.source.id,
            options,
            text: "Pay one of these costs?".to_owned(),
            can_cancel: false,
        }),
    }
}
