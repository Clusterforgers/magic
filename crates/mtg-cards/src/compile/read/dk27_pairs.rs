//! "As an additional cost to cast this spell, you may remove any number of +1/+1 counters from among creatures you
//! control. This spell costs {2} less to cast for each counter removed this way.": a required cost of X things (X may be
//! 0) and the spell's own reduction for each of them, read into the two abilities `ep22_pairs` writes.

use super::costs;
use super::tokens::{Reader, tokenize};
use crate::ability::{Ability, CostChange};
use crate::mana::ManaCostX;
use crate::refs::{Pred, Selector};
use crate::value::Amount;

/// The verbs of a cost the caster pays "any number of" times.
const VERBS: [&str; 4] = ["sacrifice", "exile", "discard", "remove"];

/// The two abilities of the clause, or `None` for another text; the caller checks that they write back as the text.
pub(super) fn dk27_any_number_less(text: &str) -> Option<Vec<Ability>> {
    let (head, tail) = text.split_once(". ")?;
    let body = head
        .strip_prefix("As an additional cost to cast this spell, you may ")
        .or_else(|| head.strip_prefix("As an additional cost to cast ~, you may "))?;
    let (verb, rest) = body.split_once(" any number of ")?;
    VERBS.contains(&verb).then_some(())?;
    let first = format!("As an additional cost to cast this spell, {verb} X {rest}.");
    let tokens = tokenize(&first);
    let mut reader = Reader::new(&tokens, true);
    let cost = costs::additional(&mut reader)?;
    (reader.sentence_end() && reader.end()).then_some(())?;
    let Ability::AdditionalCost { required: true, .. } = &cost else {
        return None;
    };
    let less = tail
        .strip_prefix("This spell costs {")
        .or_else(|| tail.strip_prefix("~ costs {"))?;
    let (generic, _) = less.split_once("} less to cast for each ")?;
    let generic: u32 = generic.parse().ok().filter(|n| *n > 0)?;
    let mana: ManaCostX = format!("{{{generic}}}").parse().ok()?;
    Some(vec![
        cost,
        Ability::CostModifier {
            applies_to: Selector(vec![Pred::This]),
            change: CostChange::Reduce,
            mana,
            per: Amount::X,
            condition: None,
            targeting: None,
            activating: None,
            least: 0,
            life: 0,
        },
    ])
}
