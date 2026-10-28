//! The kind of counter each permanent picked for "Remove a counter from a permanent you control" loses (CR 601.2h).
//!
//! Once every pick of the cost is made, each permanent picked for a part that removes a counter of any kind gets its
//! kind in the order of the picks: one it holds alone is taken, and between several the player is asked with
//! `ChooseOption`. The kinds are kept in `Choices.cost_taken.kinds` and paid with the other parts (`cast.rs`).

use mtg_core::ids::ObjectId;

use super::{Ctx, Next, Work};
use crate::choose::{ChoiceOption, OptionOptions, Target};
use crate::counters::kinds_held;
use crate::decision::DecisionKind;
use crate::parts::Part;
use crate::price::Price;

/// Settles the kinds still missing, or asks for the next one; `None` once every permanent has its kind.
pub(super) fn w2f_run(ctx: &Ctx<'_>, work: &mut Work<'_>, price: &Price) -> Option<Next> {
    let x = work.choices.x.unwrap_or(0);
    let picks = work.choices.cost_picks();
    let mut needed: Vec<ObjectId> = Vec::new();
    let mut taken = picks.iter();
    for part in &price.parts {
        let Part::Pick(pick) = part.at(x) else { continue };
        let picked = taken.next()?;
        if pick.w2f_any_kind() {
            needed.extend(picked.iter().copied());
        }
    }
    let settled = work.choices.cost_kinds().len();
    for object in needed.into_iter().skip(settled) {
        let on = Target::Object(ctx.state.object(object)?.reference());
        let kinds: Vec<_> = kinds_held(ctx.state, on).into_iter().map(|(kind, _)| kind).collect();
        let index = match (kinds.len(), work.done.w2f_kind_picked.take()) {
            (0, _) => return Some(Next::Drop),
            (1, _) => 0,
            (_, Some(index)) => index,
            (_, None) => {
                let name = ctx.state.object(object)?.card().name.clone();
                return Some(ctx.ask(DecisionKind::ChooseOption(OptionOptions {
                    source: ctx.source,
                    options: kinds
                        .iter()
                        .map(|kind| ChoiceOption::Label(format!("{} counter", kind.name())))
                        .collect(),
                    text: format!("Choose a counter to remove from {name}."),
                })));
            }
        };
        work.choices.w2f_kind_for_cost(kinds.get(index)?.clone());
    }
    None
}
