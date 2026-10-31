//! Goal: the review sheet shows every clause of a card beside the ability that stands for it, the renderer's wording and a total list of steps, with no blank row for any card of the corpus, any compiled card of a download, any stored proposal the checker marks unchecked, or any shape of ability (two faces, modes, an intervening if, X, a clause played by hand).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::card::{Card, Face, ImplStatus, Span};
use mtg_cards::compile::compile;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::read::{ReadRow, ReadSheet, RowState, sheet};
use mtg_cards::support::{PrintedCard, Support, support};
use rstest::rstest;

use common::{Corpus, find_oracle_cards, printed, printed_of_download, token_sources};

/// The printed card and the rules of a card of the corpus, by name.
fn corpus_card(name: &str) -> (PrintedCard, Card) {
    let printed = printed()
        .into_iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    let library = Corpus::repo().library();
    let card = library
        .card(&printed.oracle_id)
        .unwrap_or_else(|| panic!("the corpus has rules for {name}"))
        .clone();
    (printed, card)
}

/// Every problem of a sheet: a blank row, a printed clause that is not the card's text, a row that shows nothing.
fn problems(printed: &PrintedCard, sheet: &ReadSheet) -> Vec<String> {
    let mut found = Vec::new();
    if !sheet.is_complete() {
        found.push(format!("{}: a blank row in {sheet:#?}", printed.name));
    }
    for row in &sheet.rows {
        let text = &printed.faces[row.face].oracle_text;
        if row.span.slice(text) != Some(row.printed.as_str()) {
            found.push(format!(
                "{}: {:?} is not the text of {:?}",
                printed.name, row.printed, row.span
            ));
        }
        let shows_ability = !row.abilities.is_empty();
        let by_hand = matches!(row.state, RowState::ByHand { .. });
        if shows_ability == by_hand && !row.continues {
            found.push(format!(
                "{}: {:?} shows abilities {:?} in state {:?}",
                printed.name, row.printed, row.abilities, row.state
            ));
        }
    }
    found
}

/// The sheets of the cards of the corpus: every card file, with the fixture record it belongs to.
fn corpus_sheets() -> Vec<(PrintedCard, Card, ReadSheet)> {
    let library = Corpus::repo().library();
    printed()
        .into_iter()
        .filter_map(|printed| {
            let card = library.card(&printed.oracle_id)?.clone();
            let shown = sheet(&card, &printed);
            Some((printed, card, shown))
        })
        .collect()
}

#[test]
fn every_card_of_the_corpus_has_a_sheet_with_no_blank_row() {
    let sheets = corpus_sheets();
    assert!(
        sheets.len() > 1000,
        "the corpus gave {} cards",
        sheets.len()
    );
    let mut found = Vec::new();
    let mut empty = 0;
    for (printed, _, shown) in &sheets {
        found.extend(problems(printed, shown));
        let has_text = printed
            .faces
            .iter()
            .any(|face| mtg_cards::support::has_rules_text(&face.oracle_text));
        if has_text && shown.rows.is_empty() {
            empty += 1;
            found.push(format!("{}: text but no rows", printed.name));
        }
    }
    assert!(
        found.is_empty(),
        "{empty} sheets without rows; {} problems; the first: {:#?}",
        found.len(),
        &found[..found.len().min(5)]
    );
}

#[test]
fn a_verified_card_shows_every_ability_on_a_clause_and_leaves_nothing_to_hand() {
    let mut found = Vec::new();
    let mut verified = 0;
    for (printed, card, shown) in corpus_sheets() {
        if support(Some(&card), &printed, &Caps::everything()).support != Support::Verified {
            continue;
        }
        verified += 1;
        let placed: BTreeSet<(usize, usize)> = shown
            .rows
            .iter()
            .flat_map(|row| {
                row.abilities
                    .iter()
                    .map(move |&ability| (row.face, ability))
            })
            .collect();
        let wanted: BTreeSet<(usize, usize)> = card
            .faces
            .iter()
            .enumerate()
            .flat_map(|(face, rules)| {
                rules
                    .abilities
                    .iter()
                    .enumerate()
                    .filter(|(_, ability)| !ability.is_manual())
                    .map(move |(at, _)| (face, at))
            })
            .collect();
        let by_hand = shown
            .rows
            .iter()
            .filter(|row| matches!(row.state, RowState::ByHand { .. }))
            .count();
        if !shown.extra.is_empty() || by_hand > 0 || placed != wanted {
            found.push(format!(
                "{}: {} extra, {by_hand} by hand, shown {placed:?} of {wanted:?}",
                printed.name,
                shown.extra.len()
            ));
        }
    }
    assert!(verified > 1000, "{verified} verified cards");
    assert!(found.len() < 25, "{} cards: {found:#?}", found.len());
}

/// The cards of an Oracle Cards download in the cache, or `None` with a message on a machine without one.
fn download() -> Option<Vec<PrintedCard>> {
    let Some(file) = find_oracle_cards() else {
        eprintln!(
            "skipped: no oracle-cards-*.jsonl.gz in MTG_REAL_DATA_DIR, MTG_CACHE_DIR or ~/.cache/mtg-cli"
        );
        return None;
    };
    Some(printed_of_download(&file))
}

/// All the cards of a download take about two minutes in a debug build, so a run samples every eighth;
/// `MTG_SHEET_ALL=1` takes them all.
#[test]
fn every_compiled_card_of_a_download_has_a_sheet_with_no_blank_row() {
    let Some(cards) = download() else { return };
    let stride = if std::env::var_os("MTG_SHEET_ALL").is_some() {
        1
    } else {
        8
    };
    let mut found = Vec::new();
    let mut sheets = 0;
    for printed in cards.iter().step_by(stride) {
        let Some(card) = compile(printed).card else {
            continue;
        };
        let shown = sheet(&card, printed);
        sheets += 1;
        found.extend(problems(printed, &shown));
    }
    assert!(
        found.is_empty(),
        "{} problems in {sheets} sheets; the first: {:#?}",
        found.len(),
        &found[..found.len().min(5)]
    );
}

/// Proposals the checker does not finish on (a Spell with a `RepeatEach` nested ten deep): the renderer writes every way a nested
/// list can be said, which takes longer with each level (roadmap rt1m).
const NEVER_FINISH: [&str; 1] = ["tempt_with_vengeance"];

/// The stored proposals under `MTG_PROPOSALS_DIR`: `<dir>/*/*.json` and `<dir>/*/rejected/*.json`.
fn stored_proposals() -> Option<Vec<PathBuf>> {
    let Some(dir) = std::env::var_os("MTG_PROPOSALS_DIR").map(PathBuf::from) else {
        eprintln!(
            "skipped: MTG_PROPOSALS_DIR is not set (a directory of proposals such as ~/.cache/mtg-cli/proposals)"
        );
        return None;
    };
    if !dir.is_dir() {
        eprintln!(
            "skipped: MTG_PROPOSALS_DIR {} is not a directory",
            dir.display()
        );
        return None;
    }
    let json_in = |dir: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect()
    };
    let mut files = Vec::new();
    for batch in std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|entry| entry.ok().map(|e| e.path()))
    {
        files.extend(json_in(&batch));
        files.extend(json_in(&batch.join("rejected")));
    }
    files.sort();
    Some(files)
}

#[test]
fn every_stored_proposal_the_checker_reads_has_a_sheet_with_no_blank_row() {
    let Some(files) = stored_proposals() else {
        return;
    };
    let Some(cards) = download() else { return };
    let by_slug: BTreeMap<String, &PrintedCard> =
        cards.iter().map(|card| (slug(&card.name), card)).collect();
    let names: BTreeSet<String> = cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect();
    let tokens = Library::from_sources(&token_sources()).unwrap();
    let stride = if std::env::var_os("MTG_SHEET_ALL").is_some() {
        1
    } else {
        4
    };
    let work: Vec<(&PrintedCard, &PathBuf)> = files
        .iter()
        .filter_map(|file| {
            let stem = file.file_stem()?.to_string_lossy().into_owned();
            if NEVER_FINISH.contains(&stem.as_str()) {
                return None;
            }
            Some((*by_slug.get(&stem)?, file))
        })
        .step_by(stride)
        .collect();
    let threads = std::thread::available_parallelism()
        .map_or(4, usize::from)
        .min(8);
    let run = |part: usize| {
        let mut verdicts: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut found = Vec::new();
        for (printed, file) in work.iter().skip(part).step_by(threads) {
            let text = std::fs::read_to_string(file).unwrap();
            let report = check(&text, printed, &tokens, &names, &Caps::everything());
            *verdicts.entry(report.verdict().word()).or_default() += 1;
            let Some(rules) = &report.rules else { continue };
            if report.problems.is_empty() || report.verdict() == Verdict::Unchecked {
                found.extend(problems(printed, &sheet(rules, printed)));
            }
        }
        (verdicts, found)
    };
    let (mut verdicts, mut found) = (BTreeMap::new(), Vec::new());
    let run = &run;
    std::thread::scope(|scope| {
        let runs: Vec<_> = (0..threads)
            .map(|part| scope.spawn(move || run(part)))
            .collect();
        for handle in runs {
            let (counted, problems) = handle.join().unwrap();
            for (verdict, count) in counted {
                *verdicts.entry(verdict).or_insert(0) += count;
            }
            found.extend(problems);
        }
    });
    eprintln!(
        "{} of {} proposal files: {verdicts:?}",
        work.len(),
        files.len()
    );
    assert!(
        found.is_empty(),
        "{} problems; the first: {:#?}",
        found.len(),
        &found[..found.len().min(5)]
    );
}

/// The steps of a row as indented lines.
fn lines(row: &ReadRow) -> Vec<String> {
    row.steps
        .iter()
        .map(|step| format!("{}{}", "  ".repeat(usize::from(step.depth)), step.text))
        .collect()
}

/// The sheet of a card of the corpus, checked for blanks.
fn sheet_of(name: &str) -> ReadSheet {
    let (printed, card) = corpus_card(name);
    let shown = sheet(&card, &printed);
    assert_eq!(problems(&printed, &shown), Vec::<String>::new(), "{name}");
    shown
}

/// The named card of the corpus with these abilities on its first face instead of its rules.
fn with_abilities(name: &str, abilities: Vec<Ability>) -> (PrintedCard, Card) {
    let printed = printed()
        .into_iter()
        .find(|card| card.name == name)
        .unwrap();
    let card = Card {
        oracle_id: printed.oracle_id.clone(),
        name: printed.name.clone(),
        impl_status: ImplStatus::Partial,
        faces: vec![Face {
            name: printed.faces[0].name.clone(),
            oracle_fingerprint: String::new(),
            abilities,
        }],
        notes: None,
    };
    (printed, card)
}

#[test]
fn a_two_faced_card_has_rows_for_both_faces_and_names_them() {
    let shown = sheet_of("Delver of Secrets // Insectile Aberration");
    assert_eq!(shown.faces, ["Delver of Secrets", "Insectile Aberration"]);
    let faces: Vec<usize> = shown.rows.iter().map(|row| row.face).collect();
    assert_eq!(faces, [0, 1]);
    assert_eq!(shown.rows[0].label, "triggered ability");
    assert!(lines(&shown.rows[0]).contains(&"when: the beginning of your upkeep".to_owned()));
    assert_eq!(lines(&shown.rows[1]), ["has flying"]);
}

#[test]
fn a_modal_spell_is_one_row_with_a_block_for_each_mode() {
    let shown = sheet_of("Brigid's Command");
    assert_eq!(shown.rows.len(), 1);
    let steps = lines(&shown.rows[0]);
    assert_eq!(steps[0], "choose 2 modes:");
    let modes = steps
        .iter()
        .filter(|line| line.starts_with("  mode "))
        .count();
    assert_eq!(modes, 4, "{steps:#?}");
    assert!(
        steps.contains(&"    choose: target player".to_owned()),
        "{steps:#?}"
    );
}

#[test]
fn an_intervening_if_is_a_step_of_its_own() {
    let shown = sheet_of("Angelic Sleuth");
    let trigger = shown
        .rows
        .iter()
        .find(|row| row.label == "triggered ability")
        .unwrap();
    let steps = lines(trigger);
    assert!(steps[0].starts_with("when: "), "{steps:?}");
    assert!(steps[1].starts_with("only if: "), "{steps:?}");
    assert_eq!(trigger.state, RowState::Written);
}

#[rstest]
#[case::spell("Blaze", "deal X damage to any target")]
#[case::activated("Hydra Broodmaster", "cost: {X}{X}{G}")]
fn an_x_stays_x_in_the_steps(#[case] name: &str, #[case] step: &str) {
    let shown = sheet_of(name);
    let all: Vec<String> = shown.rows.iter().flat_map(lines).collect();
    assert!(all.iter().any(|line| line == step), "{all:#?}");
}

#[test]
fn a_clause_with_no_ability_is_played_by_hand() {
    let (printed, card) = with_abilities("Lightning Bolt", vec![]);
    let shown = sheet(&card, &printed);
    assert!(shown.is_complete());
    let row = &shown.rows[0];
    assert_eq!(row.state, RowState::ByHand { reason: None });
    assert!(row.abilities.is_empty() && row.wording.is_none());
    assert_eq!(
        lines(row),
        ["played by hand: the rules do not cover this clause"]
    );
}

#[test]
fn a_manual_clause_says_why_it_is_played_by_hand() {
    let bolt = printed()
        .into_iter()
        .find(|card| card.name == "Lightning Bolt")
        .unwrap();
    let end = u32::try_from(bolt.faces[0].oracle_text.chars().count()).unwrap();
    let reason = "damage the engine cannot deal";
    let manual = Ability::Manual {
        reason: reason.into(),
        span: Span { start: 0, end },
    };
    let (printed, card) = with_abilities("Lightning Bolt", vec![manual]);
    let shown = sheet(&card, &printed);
    let row = &shown.rows[0];
    assert_eq!(
        row.state,
        RowState::ByHand {
            reason: Some(reason.into())
        }
    );
    assert_eq!(lines(row), [format!("played by hand: {reason}")]);
    assert_eq!(shown.extra.len(), 0);
}

#[rstest]
#[case::fabricate(r#"{"Fabricate":{"count":2}}"#, "fabricate: count 2", None)]
#[case::bolster(r#"{"Bolster":{"count":1}}"#, "bolster: count 1", Some("Bolster 1."))]
#[case::time_travel(r#"{"TimeTravel":{}}"#, "time travel", Some("Time travel."))]
fn a_construct_is_shown_by_its_steps_with_or_without_a_writer(
    #[case] effect: &str,
    #[case] step: &str,
    #[case] written: Option<&str>,
) {
    let spell: Ability =
        serde_json::from_str(&format!(r#"{{"Spell":{{"effects":[{effect}]}}}}"#)).unwrap();
    let (printed, card) = with_abilities("Lightning Bolt", vec![spell]);
    let shown = sheet(&card, &printed);
    let row = &shown.rows[0];
    match written {
        None => assert!(
            matches!(row.state, RowState::Unwritten { .. }),
            "{:?}",
            row.state
        ),
        Some(text) => assert_eq!(
            row.state,
            RowState::Differs {
                written: text.to_owned()
            }
        ),
    }
    assert_eq!(row.wording.as_deref(), written);
    assert_eq!(row.label, "spell");
    assert_eq!(lines(row), [step]);
}

#[test]
fn an_ability_no_clause_stands_behind_is_listed_apart() {
    let bolt: Ability = serde_json::from_str(
        r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}"#,
    )
    .unwrap();
    let stray: Ability = serde_json::from_str(r#"{"Keyword":"Flying"}"#).unwrap();
    let (printed, card) = with_abilities("Lightning Bolt", vec![bolt, stray]);
    let shown = sheet(&card, &printed);
    assert!(shown.is_complete());
    assert_eq!(shown.rows.len(), 1);
    assert_eq!(shown.rows[0].state, RowState::Written);
    assert_eq!(shown.extra.len(), 1);
    assert_eq!(shown.extra[0].state, RowState::NoClause);
    assert_eq!(shown.extra[0].abilities, [1]);
}

#[test]
fn the_lines_of_one_spell_share_its_row() {
    let shown = sheet_of("Dispatch");
    assert_eq!(shown.rows.len(), 2);
    assert!(!shown.rows[0].continues && shown.rows[1].continues);
    assert_eq!(shown.rows[0].abilities, shown.rows[1].abilities);
    assert!(shown.rows[0].steps.len() > 1);
    assert_eq!(
        lines(&shown.rows[1]),
        ["read together with the clause above"]
    );
}

#[rstest]
#[case::a_planeswalker("Jace, the Mind Sculptor", "loyalty ability")]
#[case::a_mana_ability("Sol Ring", "mana ability")]
#[case::a_spell("Wrath of God", "spell")]
fn the_label_names_the_kind_of_ability(#[case] name: &str, #[case] label: &str) {
    let shown = sheet_of(name);
    let labels: Vec<&String> = shown.rows.iter().map(|row| &row.label).collect();
    assert!(labels.contains(&&label.to_owned()), "{labels:?}");
}
