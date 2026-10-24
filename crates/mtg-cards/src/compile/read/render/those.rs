//! Wordings the oracle text uses for the objects an earlier step named, which the data repeats as a selector: "each of
//! those creatures" for a group acted on before, "each of them" for the attackers of an event, "For each ... exiled this
//! way, create ..." before the count it uses and "for each opponent you have".

use crate::ability::Ability;
use crate::trigger::TriggerPattern;

/// "tap all creatures your opponents control, then put a stun counter on each creature your opponents control": the
/// second group is the first one again, "each of those creatures".
fn those_creatures(text: &str) -> Option<String> {
    let at = text.find("all ")?;
    let after = &text[at + 4..];
    let end = after.find([',', '.']).unwrap_or(after.len());
    let group = &after[..end];
    let noun = group.split(' ').next()?;
    let single = format!("{} {}", noun.strip_suffix('s')?, group.split_once(' ')?.1);
    let later = &after[end..];
    let target = format!("each {single}");
    later.contains(&target).then(|| {
        format!(
            "{}{}",
            &text[..at + 4 + end],
            later.replacen(&target, &format!("each of those {noun}"), 1)
        )
    })
}

/// "Create a 1/1 Insect token for each artifact or land card exiled this way." also opens with the count:
/// "For each artifact or land card exiled this way, create a 1/1 Insect token."
fn counted_first(text: &str) -> Option<String> {
    let start = text.rfind(". ").map_or(0, |at| at + 2);
    let (before, last) = text.split_at(start);
    let (made, counted) = last.strip_suffix('.')?.split_once(" for each ")?;
    counted.ends_with("exiled this way").then_some(())?;
    let mut letters = made.chars();
    let first = letters.next()?.to_ascii_lowercase();
    Some(format!(
        "{before}For each {counted}, {first}{}.",
        letters.as_str()
    ))
}

/// "put a +1/+1 counter on ~. Then you gain 1 life." is also "put a +1/+1 counter on ~ and you gain 1 life".
fn joined_with_and(text: &str) -> Option<String> {
    text.contains(". Then you ")
        .then(|| text.replacen(". Then you ", " and you ", 1))
}

/// "Create a 1/1 Rat token for each opponent." is "... for each opponent you have." on a card.
fn opponents_you_have(text: &str) -> Option<String> {
    text.contains(" for each opponent.")
        .then(|| text.replacen(" for each opponent.", " for each opponent you have.", 1))
}

/// The attackers of a batch are "each of them" after "on it".
fn each_of_them(ability: &Ability, text: &str) -> Option<String> {
    let batch = matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::GroupAttacks { .. },
            ..
        }
    );
    (batch && text.ends_with(" counter on it."))
        .then(|| text.replacen(" on it.", " on each of them.", 1))
}

/// Adds to `texts` the wordings that say again who an earlier step named.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let found: Vec<String> = texts
        .iter()
        .flat_map(|text| {
            [
                those_creatures(text),
                counted_first(text),
                joined_with_and(text),
                opponents_you_have(text),
                each_of_them(ability, text),
            ]
        })
        .flatten()
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}
