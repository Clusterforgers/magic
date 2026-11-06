//! A static ability of the attached permanent and a second ability about the same permanent, in one sentence: "Enchanted
//! creature gets +1/+0 and can't be blocked.", "Enchanted creature gets +2/+2, has vigilance, and can't attack you or
//! planeswalkers you control.", "Enchanted creature can't attack or block, and its activated abilities can't be
//! activated." Each half is written from its own ability; only the subject they share is said once.

use super::render::written_variants;
use crate::ability::Ability;
use crate::refs::{Ref, ZoneKind};

/// The subjects a sentence about the attached permanent opens with.
const SUBJECTS: [&str; 8] = [
    "Enchanted creature ",
    "Enchanted permanent ",
    "Enchanted land ",
    "Enchanted artifact ",
    "Equipped creature ",
    "Equipped permanent ",
    "Equipped land ",
    "Equipped artifact ",
];

/// Rules of the attached permanent no writer says alone, each with its predicate after the subject; the probe card is
/// named first.
const BACKS: &[(&str, &str)] = &[
    // Gauntlets of Light
    (
        r#"{"Rule":{"rule":{"Combat":{"quirk":"AssignsByToughness","who":"Attached"}}}}"#,
        "assigns combat damage equal to its toughness rather than its power",
    ),
    // Sightless Brawler
    (
        r#"{"Rule":{"rule":{"Combat":{"quirk":"CantAttackAlone","who":"Attached"}}}}"#,
        "can't attack alone",
    ),
    // Street Savvy
    (
        r#"{"Rule":{"rule":{"Combat":{"quirk":"BlockIgnoringLandwalk","who":"Attached"}}}}"#,
        "can block creatures with landwalk abilities as though they didn't have those abilities",
    ),
    // Spectral Shield
    (
        r#"{"Rule":{"rule":{"Cant":{"action":{"BeTargeted":["Spell"]},"who":"Attached"}}}}"#,
        "can't be the target of spells",
    ),
    // Tormentor's Trident
    (
        r#"{"Rule":{"rule":{"MustAttack":{"Each":[{"Is":"Attached"}]}}}}"#,
        "attacks each combat if able",
    ),
];

/// Rules of the attached permanent said as a clause of their own after a comma, with the subject the sentence must have
/// (any subject when empty).
const TAILS: &[(&str, &str, &str)] = &[
    // Petrify
    (
        r#"{"Rule":{"rule":{"Cant":{"action":{"ActivateKind":{"kind":"Every","sources":[{"Is":"Attached"}]}},"who":"EachPlayer"}}}}"#,
        "",
        "its activated abilities can't be activated",
    ),
    // Arrest: "creature" in the sources is the noun of the subject.
    (
        r#"{"Rule":{"rule":{"Cant":{"action":{"ActivateKind":{"kind":"Every","sources":[{"Type":"Creature"},{"Is":"Attached"}]}},"who":"EachPlayer"}}}}"#,
        "Enchanted creature ",
        "its activated abilities can't be activated",
    ),
    // Faith's Fetters: the attached permanent is the subject of its own activations, mana abilities aside.
    (
        r#"{"Rule":{"rule":{"Cant":{"action":{"Activate":[]},"who":{"Each":[{"Is":"Attached"}]}}}}}"#,
        "",
        "its activated abilities can't be activated unless they're mana abilities",
    ),
    // Indrik Umbra
    (
        r#"{"Rule":{"rule":{"Combat":{"quirk":{"MustBeBlocked":"Every"},"who":"Attached"}}}}"#,
        "",
        "all creatures able to block it do so",
    ),
];

/// Every wording of the two abilities as one sentence; empty unless the first is a static ability of the attached
/// permanent without a condition and the second says something of the same subject.
pub(super) fn r6b_texts(first: &Ability, second: &Ability) -> Vec<String> {
    let Ability::Static {
        affected: Ref::Attached,
        condition: None,
        defining: false,
        from: ZoneKind::Battlefield,
        ..
    } = first
    else {
        return Vec::new();
    };
    // Only a static ability or a rule says something more of the attached permanent.
    if !matches!(second, Ability::Static { .. } | Ability::Rule { .. }) {
        return Vec::new();
    }
    let second_value = serde_json::to_value(second).ok();
    let row = |json: &str| serde_json::from_str::<serde_json::Value>(json).ok() == second_value;
    let back_row = BACKS
        .iter()
        .find(|(json, _)| row(json))
        .map(|(_, back)| *back);
    let tail_row = TAILS.iter().find(|(json, ..)| row(json));
    let backs = written_variants(second);
    let mut texts = Vec::new();
    for front in fronts(first) {
        let Some((subject, front)) = split_subject(&front) else {
            continue;
        };
        let front = front.strip_suffix('.').unwrap_or(front);
        if let Some((_, wanted, tail)) = tail_row
            && (wanted.is_empty() || *wanted == subject)
        {
            texts.push(format!("{subject}{front}, and {tail}."));
        }
        let said = backs.iter().filter_map(|back| {
            split_subject(back)
                .filter(|(same, _)| *same == subject)
                .map(|(_, back)| back.strip_suffix('.').unwrap_or(back))
        });
        for back in said.chain(back_row) {
            texts.push(format!("{subject}{front} and {back}."));
            // Two statics apply together, in either order: "can't be blocked and has shroud".
            if matches!(second, Ability::Static { .. }) && !front.contains(" and ") {
                texts.push(format!("{subject}{back} and {front}."));
            }
            // "gets +2/+2 and has vigilance" with a third part is a list: "gets +2/+2, has vigilance, and ...".
            if let Some((head, last)) = front.rsplit_once(" and ")
                && !head.contains(" and ")
            {
                texts.push(format!("{subject}{head}, {last}, and {back}."));
            }
        }
    }
    texts
}

/// The wordings of the static ability, with "Enchanted permanent" beside "Enchanted creature": the noun only repeats
/// what the Aura enchants, which its enchant keyword says.
fn fronts(first: &Ability) -> Vec<String> {
    let mut texts = written_variants(first);
    let permanent: Vec<String> = texts
        .iter()
        .filter_map(|text| text.strip_prefix("Enchanted creature "))
        .map(|rest| format!("Enchanted permanent {rest}"))
        .collect();
    for text in permanent {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
    texts
}

/// The subject a wording opens with and the rest of it.
fn split_subject(text: &str) -> Option<(&'static str, &str)> {
    SUBJECTS
        .iter()
        .find_map(|subject| text.strip_prefix(subject).map(|rest| (*subject, rest)))
}
