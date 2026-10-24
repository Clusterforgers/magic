//! A count said with "where X is the number of": "creatures you control get +X/+X until end of turn, where X is the
//! number of cards in your hand." and "Destroy each nonland permanent with mana value equal to the number of charge
//! counters on this land." The renderers write "get +1/+1 ... for each card in your hand" and "with mana value X, where
//! X is the number of ...".

use super::text::{json, plural, singular};
use crate::ability::Ability;

/// The sizes a pump with a count can have, as the renderers write them and as the card writes them.
const SIZES: [(&str, &str); 3] = [("+1/+1", "+X/+X"), ("+1/+0", "+X/+0"), ("-1/-1", "-X/-X")];

/// Adds the "where X is" wordings of a pump by a count and of a destruction by a count of counters.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let rules = json(ability);
    let mut said = Vec::new();
    if rules.contains(r#""ModifyPT""#) && rules.contains(r#""Count""#) {
        said.extend(texts.iter().filter_map(|text| pump(text)));
    }
    if rules.contains(r#""Destroy""#) && rules.contains(r#""Counters""#) {
        said.extend(texts.iter().filter_map(|text| destroy(text)));
    }
    texts.extend(said);
}

/// "... get +1/+1 until end of turn for each card in your hand." as the pump by X.
fn pump(text: &str) -> Option<String> {
    let (before, after) = text.split_once(" until end of turn for each ")?;
    let (per, tail) = after.split_at(after.find('.').unwrap_or(after.len()));
    let (front, size) = SIZES
        .iter()
        .find_map(|(small, large)| before.strip_suffix(small).map(|front| (front, *large)))?;
    Some(format!(
        "{front}{size} until end of turn, where X is the number of {}{tail}",
        plural(per)
    ))
}

/// "Destroy all nonland permanents with mana value X, where X is the number of charge counters on ~." as the destruction
/// of each permanent with a mana value equal to the count.
fn destroy(text: &str) -> Option<String> {
    let at = text.find("Destroy all ")?;
    let rest = &text[at + "Destroy all ".len()..];
    let (kind, count) = rest.split_once(" with mana value X, where X is the number of ")?;
    Some(format!(
        "{}Destroy each {} with mana value equal to the number of {count}",
        &text[..at],
        singular(kind)
    ))
}
