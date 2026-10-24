//! A group of cards named by several subtypes that a static ability gives something: the cards say "Merfolk and Druid cards
//! in your graveyard have retrace" where the writer says "Merfolk or Druid cards".

use mtg_core::card::Subtype;

use crate::ability::Ability;
use crate::refs::{Pred, Ref};

/// "A or B", "A, B, or C" and the same with "and".
fn listed(names: &[&str], word: &str) -> String {
    match names {
        [first, second] => format!("{first} {word} {second}"),
        [rest @ .., last] => format!("{}, {word} {last}", rest.join(", ")),
        [] => String::new(),
    }
}

/// The wordings of a static ability over a group of cards with "and" between the subtypes.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Static {
        affected: Ref::Each(selector),
        ..
    } = ability
    else {
        return;
    };
    for pred in &selector.0 {
        let Pred::AnyOf(list) = pred else {
            continue;
        };
        let names: Option<Vec<&str>> = list
            .iter()
            .map(|one| match one {
                Pred::Subtype(Subtype(name)) => Some(name.as_str()),
                _ => None,
            })
            .collect();
        let Some(names) = names.filter(|names| names.len() > 1) else {
            continue;
        };
        let (or, and) = (
            format!("{} cards", listed(&names, "or")),
            format!("{} cards", listed(&names, "and")),
        );
        let found: Vec<String> = texts
            .iter()
            .filter(|text| text.contains(&or))
            .map(|text| text.replace(&or, &and))
            .collect();
        texts.extend(found);
    }
}
