//! Goal: over the design corpus the compiler reads nothing wrongly (every ability it reads is an ability of the hand-written file on the same face), the cards it reproduces whole never shrink in number, and the renderer writes the hand-written abilities as the cards' own text.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;

use serde_json::Value;

use mtg_cards::ability::Ability;
use mtg_cards::card::{Card, ImplStatus};
use mtg_cards::compile::{ClauseKind, Compiled, compile, render_variants, same_text};
use mtg_cards::load::Library;
use mtg_cards::support::PrintedCard;

use common::{cards_dir, printed};

/// The corpus cards whose compiled abilities are the hand-written file's, face by face. A card joins
/// the list when the templates reach it and never leaves; a card that matches but is not listed fails
/// the test too, so the list is the ratchet.
const REPRODUCED: [&str; 59] = [
    "Archon of Cruelty",
    "Arvad the Cursed",
    "Battlefield Forge",
    "Black Knight",
    "Blaze",
    "Bloodbraid Elf",
    "Bonesplitter",
    "Brainstorm",
    "Bronzeplate Boar",
    "Carnage Tyrant",
    "Cathars' Crusade",
    "Counterspell",
    "Cyclonic Rift",
    "Death Ward",
    "Dockside Extortionist",
    "Doom Blade",
    "Drudge Skeletons",
    "Epic Proportions",
    "Eternal Witness",
    "Faithful Watchdog",
    "Faithless Looting",
    "Favorable Winds",
    "Felidar Sovereign",
    "Flooded Strand",
    "Giant Growth",
    "Glorious Anthem",
    "Goblin Spy",
    "Hall of Triumph",
    "Hero of Bladehold",
    "Hulking Raptor",
    "Idyllic Beachfront",
    "Izzet Charm",
    "Jungle Lion",
    "Kessig Wolf Run",
    "Kongming, \"Sleeping Dragon\"",
    "Levitation",
    "Lightning Bolt",
    "Lizard Blades",
    "Llanowar Elves",
    "Mizzium Mortars",
    "Monastery Swiftspear",
    "Lotus Petal",
    "Mulldrifter",
    "Opt",
    "Pacifism",
    "Rally the Ranks",
    "Rampant Growth",
    "Rhystic Study",
    "Serra Angel",
    "Sodden Verdure",
    "Sol Ring",
    "Steam Vents",
    "Stroke of Genius",
    "Supreme Verdict",
    "Swiftfoot Boots",
    "Swords to Plowshares",
    "Tireless Tracker",
    "Warmonger Hellkite",
    "Wrath of God",
];

fn hand_written() -> Library {
    Library::load_dir(&cards_dir()).unwrap_or_else(|errors| panic!("{errors}"))
}

fn compiled_corpus() -> Vec<(PrintedCard, Compiled)> {
    printed()
        .into_iter()
        .map(|card| {
            let compiled = compile(&card);
            (card, compiled)
        })
        .collect()
}

/// An ability as data without its mode labels: a label is a short name for a prompt that a hand-written
/// file gives ("Counter", "Loot") and the compiler cannot know.
fn unlabeled(ability: &Ability) -> Value {
    fn strip(value: &mut Value) {
        match value {
            Value::Object(map) => {
                if let Some(Value::Array(modes)) = map.get_mut("modes") {
                    for mode in modes.iter_mut().filter_map(Value::as_object_mut) {
                        mode.remove("label");
                    }
                }
                map.values_mut().for_each(strip);
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(ability).unwrap();
    strip(&mut value);
    value
}

/// Whether two abilities are the same up to the labels of their modes.
fn same_ability(left: &Ability, right: &Ability) -> bool {
    unlabeled(left) == unlabeled(right)
}

/// Whether every clause is read and the compiled abilities are the file's, face by face.
fn reproduces(compiled: &Compiled, by_hand: &Card) -> bool {
    compiled.card.as_ref().is_some_and(|card| {
        card.impl_status == ImplStatus::Full
            && card.faces.len() == by_hand.faces.len()
            && card
                .faces
                .iter()
                .zip(&by_hand.faces)
                .all(|(made, written)| {
                    made.abilities.len() == written.abilities.len()
                        && made
                            .abilities
                            .iter()
                            .zip(&written.abilities)
                            .all(|(made, written)| same_ability(made, written))
                })
    })
}

#[test]
fn every_ability_the_compiler_reads_is_in_the_hand_written_file() {
    let library = hand_written();
    for (printed, compiled) in compiled_corpus() {
        let Some(card) = compiled.card else { continue };
        let by_hand = library.card(&printed.oracle_id).unwrap();
        for (face, ability) in card.abilities() {
            if !ability.is_manual() {
                assert!(
                    by_hand.faces[face]
                        .abilities
                        .iter()
                        .any(|written| same_ability(written, ability)),
                    "{} (face {face}): {ability:?} is not in the hand-written file",
                    printed.name
                );
            }
        }
    }
}

#[test]
fn the_reproduced_cards_are_exactly_the_listed_ones() {
    let library = hand_written();
    let found: BTreeSet<String> = compiled_corpus()
        .into_iter()
        .filter(|(printed, compiled)| {
            reproduces(compiled, library.card(&printed.oracle_id).unwrap())
        })
        .map(|(printed, _)| printed.name)
        .collect();
    let listed: BTreeSet<String> = REPRODUCED.iter().map(|name| (*name).to_owned()).collect();
    let lost: Vec<_> = listed.difference(&found).collect();
    let new: Vec<_> = found.difference(&listed).collect();
    assert!(lost.is_empty(), "listed but no longer reproduced: {lost:?}");
    assert!(
        new.is_empty(),
        "reproduced but not listed (add them to REPRODUCED): {new:?}"
    );
}

/// The corpus cards the compiler does not reproduce, each with the first clause it leaves unread.
/// Printed, not asserted: it shows where the next templates go (`--nocapture`).
#[test]
fn the_frontier_of_the_compiler() {
    let library = hand_written();
    let mut frontier = Vec::new();
    for (printed, compiled) in compiled_corpus() {
        if reproduces(&compiled, library.card(&printed.oracle_id).unwrap()) {
            continue;
        }
        let first = compiled
            .clauses
            .iter()
            .find(|clause| !clause.read)
            .map_or_else(
                || "(no rules text)".to_owned(),
                |clause| format!("{:?}: {}", clause.kind, clause.text.replace('\n', " / ")),
            );
        frontier.push(format!("{} -> {first}", printed.name));
    }
    eprintln!(
        "{} cards reproduced, frontier:\n{}",
        REPRODUCED.len(),
        frontier.join("\n")
    );
}

/// The clause without its ability word ("Landfall — "), which is flavor.
fn without_ability_word(text: &str) -> &str {
    match text.split_once(" \u{2014} ") {
        Some((word, rest)) if !word.contains(['~', ',', '.']) && word.split(' ').count() <= 3 => {
            rest
        }
        _ => text,
    }
}

/// Every spell, triggered or mana ability of a hand-written file that the renderer can write is
/// written as the text of the card: the clauses of its kind, on the same face.
#[test]
fn the_renderer_writes_the_hand_written_abilities_as_the_cards_text() {
    let library = hand_written();
    let mut rendered = 0;
    for (printed, compiled) in compiled_corpus() {
        let by_hand = library.card(&printed.oracle_id).unwrap();
        for (face, ability) in by_hand.abilities() {
            let written = render_variants(ability);
            if written.is_empty() {
                continue;
            }
            // The spell ability is the spell lines of the face, or its list of modes.
            let kinds: &[ClauseKind] = match ability {
                Ability::Spell { .. } => &[ClauseKind::Spell, ClauseKind::Modal],
                Ability::Triggered { .. } => &[ClauseKind::Triggered],
                Ability::Replacement { .. } | Ability::Rule { .. } | Ability::Static { .. } => {
                    &[ClauseKind::Static]
                }
                _ => &[ClauseKind::Activated],
            };
            let lines: Vec<&str> = compiled
                .clauses
                .iter()
                .filter(|clause| clause.face == face && kinds.contains(&clause.kind))
                .map(|clause| without_ability_word(&clause.text))
                .collect();
            let wanted = if matches!(ability, Ability::Spell { .. }) {
                vec![lines.join("\n")]
            } else {
                lines.iter().map(|line| (*line).to_owned()).collect()
            };
            assert!(
                wanted
                    .iter()
                    .any(|text| written.iter().any(|variant| same_text(variant, text))),
                "{}: none of {written:?} is in {wanted:?}",
                printed.name
            );
            rendered += 1;
        }
    }
    assert!(rendered >= 10, "only {rendered} abilities were rendered");
}
