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
    if let Some(found) = r.attempt(super::r371_owner::r371_attacks_unsacrificed) {
        return Some(found);
    }
    if let Some(found) = r.attempt(super::r371_owner::r371_attacks_and_cant) {
        return Some(found);
    }
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
#[allow(clippy::too_many_lines)]
pub fn texts(first: &Ability, second: &Ability) -> Vec<String> {
    let owner = super::r371_owner::r371_pair_texts(first, second);
    if !owner.is_empty() {
        return owner;
    }
    let escapes = super::r304b_writers::r304b_escapes_instead(first, second);
    if !escapes.is_empty() {
        return escapes;
    }
    let r302c = super::r302c_writers::r302c_pair_texts(first, second);
    if !r302c.is_empty() {
        return r302c;
    }
    let r285 = super::r285_writers::r285_pair_texts(first, second);
    if !r285.is_empty() {
        return r285;
    }
    let pairs305 = super::r305b_writers::r305b_pair_texts(first, second);
    if !pairs305.is_empty() {
        return pairs305;
    }
    let r303c = super::r303c_writers::r303c_pair_texts(first, second, super::written_variants);
    if !r303c.is_empty() {
        return r303c;
    }
    let alone = super::wf25_writers::wf25_pair_texts(first, second);
    if !alone.is_empty() {
        return alone;
    }
    let zones = super::ep26_mana::ep26_two_zones_texts(first, second);
    if !zones.is_empty() {
        return zones;
    }
    let held = super::c10_uc::c10_uc_enters_held_texts(first, second);
    if !held.is_empty() {
        return held;
    }
    let otherwise = super::dk1a_attached::otherwise_texts(first, second);
    if !otherwise.is_empty() {
        return otherwise;
    }
    let paid_less = super::ep22_pairs::texts(first, second);
    let paid_less = if paid_less.is_empty() {
        super::ep30_paid::ep30_paid_texts(first, second)
    } else {
        paid_less
    };
    if !paid_less.is_empty() {
        return paid_less;
    }
    let taxes = super::g11c_taxes::g11c_pair_texts(first, second);
    if !taxes.is_empty() {
        return taxes;
    }
    let together = super::fix32_pairs::texts(first, second);
    if !together.is_empty() {
        return together;
    }
    let attacking = super::attack_pairs::texts(first, second);
    if !attacking.is_empty() {
        return attacking;
    }
    let attached = super::r6b_pairs::r6b_texts(first, second);
    if !attached.is_empty() {
        return attached;
    }
    let entering = super::r284_pairs::r284_texts(first, second);
    if !entering.is_empty() {
        return entering;
    }
    let chosen = super::c4_chosen_pairs::texts(first, second);
    if !chosen.is_empty() {
        return chosen;
    }
    if let Some(one) = super::r6c_pairs::merged(first, second) {
        let written = super::render::written_variants(&one);
        if !written.is_empty() {
            return written;
        }
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
