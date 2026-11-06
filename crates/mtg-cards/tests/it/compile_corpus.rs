//! Goal: over the design corpus the compiler reads nothing wrongly (every ability it reads is an ability of the hand-written file on the same face), the cards it reproduces whole never shrink in number, and the renderer writes the hand-written abilities as the cards' own text.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;

use serde_json::Value;

use mtg_cards::ability::Ability;
use mtg_cards::card::{Card, ImplStatus};
use mtg_cards::compile::{
    ClauseKind, Compiled, compile, equip_line, escalate_cost, escalate_reads, forecast_lines,
    own_names, reinforce_line, same_text, static_pair_texts, written_variants,
};
use mtg_cards::load::Library;
use mtg_cards::support::PrintedCard;

use common::{ADDING, Corpus, UPDATE, printed};

fn hand_written() -> Library {
    Corpus::repo().library()
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

/// The ratchet list: the corpus cards whose compiled abilities are the hand-written file's, face by face, one
/// name per line in sorted order so that branches adding names merge cleanly. A card joins the list when the
/// templates reach it and never leaves; a card that matches but is not listed fails the test too.
fn list_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reproduced.txt")
}

/// The names in a list file, in file order.
fn names_in(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The list file of these names: sorted, one per line, no duplicates.
pub fn list_text(names: &BTreeSet<String>) -> String {
    names
        .iter()
        .flat_map(|name| [name.as_str(), "\n"])
        .collect()
}

/// The corpus cards whose compiled abilities are the hand-written file's.
pub fn reproduced(corpus: &Corpus) -> BTreeSet<String> {
    let library = corpus.library();
    corpus
        .printed()
        .into_iter()
        .filter(|shown| {
            library
                .card(&shown.oracle_id)
                .is_some_and(|by_hand| reproduces(&compile(shown), by_hand))
        })
        .map(|shown| shown.name)
        .collect()
}

/// What keeps the cards `found` from being exactly those in the list `text`: a listed card that is no longer
/// reproduced, a reproduced card that is not listed, a list that is not sorted without duplicates.
pub fn ratchet_problems(found: &BTreeSet<String>, text: &str) -> Vec<String> {
    let listed: BTreeSet<String> = names_in(text).into_iter().collect();
    let mut problems = Vec::new();
    let lost: Vec<_> = listed.difference(found).collect();
    if !lost.is_empty() {
        problems.push(format!(
            "listed in tests/fixtures/reproduced.txt but no longer reproduced: {lost:?}"
        ));
    }
    let new: Vec<_> = found.difference(&listed).collect();
    if !new.is_empty() {
        problems.push(format!(
            "reproduced but not listed: add {new:?} to tests/fixtures/reproduced.txt, or run `{UPDATE}` ({ADDING})"
        ));
    }
    if text != list_text(&listed) {
        problems.push(format!(
            "tests/fixtures/reproduced.txt is not one name per line, sorted, without duplicates; run `{UPDATE}`"
        ));
    }
    problems
}

#[test]
fn the_reproduced_cards_are_exactly_the_listed_ones() {
    let found = reproduced(&Corpus::repo());
    let mut text = std::fs::read_to_string(list_path()).unwrap();
    if std::env::var_os("MTG_UPDATE_CARDS").is_some() {
        // The update only adds: a card that stopped matching stays listed and fails.
        let mut listed: BTreeSet<String> = names_in(&text).into_iter().collect();
        listed.extend(found.iter().cloned());
        text = list_text(&listed);
        std::fs::write(list_path(), &text).unwrap();
    }
    let problems = ratchet_problems(&found, &text);
    assert!(problems.is_empty(), "{problems:#?}");
}

/// The corpus cards the compiler does not reproduce, each with the first clause it leaves unread.
/// Printed, not asserted: it shows where the next templates go (`--nocapture`).
#[test]
fn the_frontier_of_the_compiler() {
    let library = hand_written();
    let mut frontier = Vec::new();
    let mut reproduced = 0;
    for (printed, compiled) in compiled_corpus() {
        if reproduces(&compiled, library.card(&printed.oracle_id).unwrap()) {
            reproduced += 1;
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
        reproduced,
        frontier.join("\n")
    );
}

/// The clause without its ability word ("Landfall — "), which is flavor.
fn without_ability_word(text: &str) -> &str {
    match text.split_once(" \u{2014} ") {
        Some((word, rest)) if !word.contains(['~', ',', '.']) && word.split(' ').count() <= 5 => {
            rest
        }
        _ => text,
    }
}

/// The text of a Saga chapter without its roman numerals ("I, II — "): a line of several chapters is one ability for each.
fn without_numerals(text: &str) -> &str {
    match text.split_once(" \u{2014} ") {
        Some((word, rest))
            if word
                .chars()
                .all(|c| matches!(c, 'I' | 'V' | 'X' | ',' | ' ')) =>
        {
            rest
        }
        _ => text,
    }
}

/// When the abilities from the first one on are equip abilities with an option (a price, a limit, a reduction), the one
/// keyword line of the card they stand for is among the clauses of the face: how many abilities the line takes.
fn equip_options_are_written(
    printed: &PrintedCard,
    compiled: &Compiled,
    listed: &[(usize, &Ability)],
) -> Option<usize> {
    let face = listed.first()?.0;
    let rest: Vec<Ability> = listed
        .iter()
        .take_while(|(same, _)| *same == face)
        .map(|(_, each)| (*each).clone())
        .collect();
    let (line, count) = equip_line(&rest)?;
    let lines: Vec<&str> = compiled
        .clauses
        .iter()
        .filter(|clause| {
            clause.face == face
                && matches!(
                    clause.kind,
                    ClauseKind::Keyword | ClauseKind::Static | ClauseKind::Activated
                )
        })
        .map(|clause| without_ability_word(&clause.text))
        .collect();
    assert!(
        lines.iter().any(|text| same_text(&line, text)),
        "{}: {line:?} is not in {lines:?}",
        printed.name
    );
    Some(count)
}

/// Whether a Reinforce or Forecast ability (an activated ability from the hand) is written as a keyword line of its face;
/// `None` when the ability is not one.
fn hand_keyword_is_written(
    printed: &PrintedCard,
    compiled: &Compiled,
    face: usize,
    ability: &Ability,
) -> Option<()> {
    let mut written = forecast_lines(ability, written_variants);
    written.extend(reinforce_line(ability));
    if written.is_empty() {
        return None;
    }
    let lines: Vec<&str> = compiled
        .clauses
        .iter()
        .filter(|clause| {
            clause.face == face
                && matches!(clause.kind, ClauseKind::Keyword | ClauseKind::Activated)
        })
        .map(|clause| without_ability_word(&clause.text))
        .collect();
    assert!(
        lines.iter().any(|text| written
            .iter()
            .any(|line| same_text(without_ability_word(line), text))),
        "{}: none of {written:?} is in {lines:?}",
        printed.name
    );
    Some(())
}

/// Every spell, triggered or mana ability of a hand-written file that the renderer can write is
/// written as the text of the card: the clauses of its kind, on the same face.
#[test]
fn the_renderer_writes_the_hand_written_abilities_as_the_cards_text() {
    let library = hand_written();
    let mut rendered = 0;
    for (printed, compiled) in compiled_corpus() {
        let by_hand = library.card(&printed.oracle_id).unwrap();
        let listed: Vec<(usize, &Ability)> = by_hand.abilities().collect();
        let mut skip_next = false;
        for (at, &(face, ability)) in listed.iter().enumerate() {
            if std::mem::take(&mut skip_next) {
                continue;
            }
            // Equip abilities with an option (a price, a limit, a reduction) are one keyword line of the card.
            if let Some(count) = equip_options_are_written(&printed, &compiled, &listed[at..]) {
                // The line takes two abilities at most (`equip_line` refuses more).
                skip_next = count == 2;
                rendered += count;
                continue;
            }
            // Reinforce and Forecast are one keyword line of the card.
            if hand_keyword_is_written(&printed, &compiled, face, ability).is_some() {
                rendered += 1;
                continue;
            }
            // Escalate is a keyword line beside the modes of the spell.
            if let Some(cost) = escalate_cost(ability) {
                assert!(
                    compiled.clauses.iter().any(|clause| clause.face == face
                        && clause.kind == ClauseKind::Keyword
                        && escalate_reads(&clause.text, cost)),
                    "{}: no Escalate line for {cost:?}",
                    printed.name
                );
            }
            // Two statics with one subject may be one sentence of the card; printed as two sentences, each
            // is checked on its own below.
            if let Some(&(same, second)) = listed.get(at + 1) {
                let pair = static_pair_texts(ability, second);
                let lines: Vec<&str> = compiled
                    .clauses
                    .iter()
                    .filter(|clause| clause.face == face && clause.kind == ClauseKind::Static)
                    .map(|clause| without_numerals(without_ability_word(&clause.text)))
                    .collect();
                if same == face
                    && lines
                        .iter()
                        .any(|text| pair.iter().any(|written| same_text(written, text)))
                {
                    skip_next = true;
                    rendered += 2;
                    continue;
                }
            }
            let written = written_variants(ability);
            if written.is_empty() {
                continue;
            }
            // The spell ability is the spell lines of the face, or its list of modes.
            let kinds: &[ClauseKind] = match ability {
                Ability::Spell { .. } => &[ClauseKind::Spell, ClauseKind::Modal],
                Ability::Triggered { .. } => &[ClauseKind::Triggered],
                Ability::Loyalty { .. } => &[ClauseKind::Loyalty],
                Ability::Replacement { .. }
                | Ability::Rule { .. }
                | Ability::Static { .. }
                | Ability::AdditionalCost { .. }
                | Ability::FreeCast { .. }
                | Ability::CastOnly { .. }
                | Ability::CostModifier { .. } => &[ClauseKind::Static],
                // A keyword line the keyword reader does not read ("Unearth—Pay eight {E}.") is written whole.
                Ability::Keyword(_) => &[ClauseKind::Keyword, ClauseKind::Static],
                _ => &[ClauseKind::Activated],
            };
            let lines: Vec<&str> = compiled
                .clauses
                .iter()
                .filter(|clause| clause.face == face && kinds.contains(&clause.kind))
                .map(|clause| without_numerals(without_ability_word(&clause.text)))
                .collect();
            let wanted = if matches!(ability, Ability::Spell { .. }) {
                vec![lines.join("\n")]
            } else {
                lines.iter().map(|line| (*line).to_owned()).collect()
            };
            // The clause shows the face's own names as `~`, also inside a token name or a type.
            let names = printed.faces.get(face).map(own_names).unwrap_or_default();
            let shown = |variant: &str| {
                names.iter().fold(variant.to_owned(), |text, name| {
                    text.replace(name.as_str(), "~")
                })
            };
            assert!(
                wanted.iter().any(|text| written.iter().any(|variant| {
                    // A wording may carry the ability word that the clause loses above ("Morbid — ...").
                    let variant = without_ability_word(variant);
                    same_text(without_numerals(variant), text)
                        || same_text(without_numerals(&shown(variant)), text)
                })),
                "{}: none of {written:?} is in {wanted:?}",
                printed.name
            );
            rendered += 1;
        }
    }
    assert!(rendered >= 10, "only {rendered} abilities were rendered");
}
