//! Two activated abilities the card prints as one line with either cost: "{3}, {T} or {W}, {T}: Prevent the next 2
//! damage that would be dealt to any target this turn." is one ability for each cost, alike in everything else.

use super::render::written_variants;
use crate::ability::Ability;

/// Every wording of two activated abilities that differ only in their cost as one line ("C1 or C2: E."); empty for any
/// other two.
pub(super) fn r300c_either_cost(first: &Ability, second: &Ability) -> Vec<String> {
    let (Ability::Activated { cost, .. }, Ability::Activated { cost: other, .. }) = (first, second)
    else {
        return Vec::new();
    };
    if cost == other || !same_but_cost(first, second) {
        return Vec::new();
    }
    let seconds: Vec<(String, String)> = written_variants(second)
        .iter()
        .filter_map(|text| split(text))
        .collect();
    let mut texts = Vec::new();
    for text in written_variants(first) {
        let Some((head, rest)) = split(&text) else {
            continue;
        };
        for (other_head, other_rest) in &seconds {
            if *other_rest == rest {
                let joined = format!("{head} or {other_head}: {rest}");
                if !texts.contains(&joined) {
                    texts.push(joined);
                }
            }
        }
    }
    texts
}

/// The cost and the rest of an activated ability's wording, split at its first colon.
fn split(text: &str) -> Option<(String, String)> {
    let (head, rest) = text.split_once(": ")?;
    (!head.is_empty() && !rest.is_empty()).then(|| (head.to_owned(), rest.to_owned()))
}

/// Whether the two abilities are equal once the second takes the first's cost.
fn same_but_cost(first: &Ability, second: &Ability) -> bool {
    let (Ability::Activated { cost, .. }, Ability::Activated { .. }) = (first, second) else {
        return false;
    };
    let mut second = second.clone();
    if let Ability::Activated { cost: other, .. } = &mut second {
        other.clone_from(cost);
    }
    *first == second
}
