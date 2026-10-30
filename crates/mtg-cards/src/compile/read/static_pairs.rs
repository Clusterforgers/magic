//! Two statics in one sentence after one condition: "As long as you control your commander, this creature gets +2/+2 and
//! creatures you control have vigilance." is a bonus for the card and a keyword for a group, both under the condition,
//! and the sentence reads into the two abilities in order and writes back as the one sentence.

use super::amounts::counted_stats;
use super::as_long_as;
use super::effects::power_toughness;
use super::render::capitalize;
use super::statics::{leading_condition, predicate, predicate_mods, statics, subjects};
use super::tokens::Reader;
use crate::ability::Ability;
use crate::modify::Modification;
use crate::refs::{Ref, ZoneKind};

/// The two statics of the sentence, when the card gets a bonus and a group gets a bonus or a keyword.
pub(super) fn read(r: &mut Reader<'_>) -> Option<Vec<Ability>> {
    if let Some(found) = r.attempt(super::c10_uc::c10_uc_enters_held) {
        return Some(found);
    }
    if let Some(found) = r.attempt(super::dk1a_attached::otherwise_read) {
        return Some(found);
    }
    let condition = leading_condition(r)?;
    r.source().then_some(())?;
    r.word("gets").then_some(())?;
    let stats = power_toughness(r)?;
    let first = vec![counted_stats(r, stats)?];
    r.word("and").then_some(())?;
    let (group, plural) = super::statics::subject(r, false)?;
    let second = predicate_mods(r, plural).filter(|_| plural)?;
    r.sentence_end().then_some(())?;
    Some(vec![
        statics(Ref::This, first, Some(condition.clone())),
        statics(group, second, Some(condition)),
    ])
}

/// Every wording of two statics that read as one sentence, with a capital and a period; empty for any other two
/// abilities.
pub fn texts(first: &Ability, second: &Ability) -> Vec<String> {
    let held = super::c10_uc::c10_uc_enters_held_texts(first, second);
    if !held.is_empty() {
        return held;
    }
    let otherwise = super::dk1a_attached::otherwise_texts(first, second);
    if !otherwise.is_empty() {
        return otherwise;
    }
    let attacking = super::attack_pairs::texts(first, second);
    if !attacking.is_empty() {
        return attacking;
    }
    let chosen = super::c4_chosen_pairs::texts(first, second);
    if !chosen.is_empty() {
        return chosen;
    }
    let (
        Ability::Static {
            affected: Ref::This,
            mods: first_mods,
            condition: Some(held),
            defining: false,
            from: ZoneKind::Battlefield,
        },
        Ability::Static {
            affected: group,
            mods: second_mods,
            condition: Some(same),
            defining: false,
            from: ZoneKind::Battlefield,
        },
    ) = (first, second)
    else {
        return Vec::new();
    };
    if held != same || !matches!(first_mods.as_slice(), [Modification::ModifyPT { .. }]) {
        return Vec::new();
    }
    let Some(front) = predicate(first_mods, false, true, true) else {
        return Vec::new();
    };
    let lowered = |subject: &str| {
        let mut chars = subject.chars();
        chars.next().map_or_else(String::new, |first| {
            first.to_lowercase().chain(chars).collect()
        })
    };
    let mut written = Vec::new();
    for held in as_long_as::texts(held) {
        for (subject, plural) in subjects(group, false) {
            let Some(back) = predicate(second_mods, plural, false, true).filter(|_| plural) else {
                continue;
            };
            let sentence = format!(
                "as long as {held}, ~ {front} and {} {back}",
                lowered(&subject)
            );
            written.push(format!("{}.", capitalize(&sentence)));
        }
    }
    written
}
