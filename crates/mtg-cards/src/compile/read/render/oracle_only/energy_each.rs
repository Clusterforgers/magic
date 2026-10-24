//! Energy gained for each thing: "Whenever this creature enters or attacks, you get {E} for each creature you control."
//! The renderers word the count with a "where" clause.

use crate::ability::Ability;
use crate::compile::read::amounts::counted_parts;
use crate::compile::read::costs::energy_text;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::CounterKind;

/// The words that name the amount of energy, up to the end of the sentence.
const AMOUNT: &str = "you get an amount of {E} equal to ";

/// Adds the wordings that give the energy of each thing counted.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let counted = ability.effects().iter().find_map(|effect| match effect {
        Effect::PutCounters {
            on: Ref::You,
            counter: CounterKind::Other(name),
            count,
        } if name == "energy" => counted_parts(count),
        _ => None,
    });
    let Some((each, tail)) = counted else {
        return;
    };
    let Some(energy) = energy_text(&each) else {
        return;
    };
    let found: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let from = text.find(AMOUNT)?;
            let end = from + text[from..].find('.')?;
            Some(format!(
                "{}you get {energy} {tail}{}",
                &text[..from],
                &text[end..]
            ))
        })
        .collect();
    texts.extend(found);
}
