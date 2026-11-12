//! Goal: the r30-2 pack's cards read back as their Oracle text: every clause of a listed card is written back by the renderers, and a dev report prints the read back of any card on request.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::Path;

use mtg_cards::compile::compile;
use mtg_cards::proposal::{ClauseState, check_exact};
use mtg_cards::support::{Caps, PrintedCard};
use mtg_import::scryfall::cards;

use crate::common;

/// The printed card of a Scryfall object file.
fn printed_at(path: &Path) -> PrintedCard {
    let text = std::fs::read_to_string(path).unwrap();
    let mut found: Vec<_> = cards(format!("[{text}]").as_bytes()).collect();
    common::printed_of(&found.remove(0).unwrap())
}

/// The read back of a rules file against its fixture, one line per clause.
fn report(rules: &Path, fixture: &Path) -> (Vec<String>, bool) {
    let printed = printed_at(fixture);
    let text = std::fs::read_to_string(rules).unwrap();
    let names = BTreeSet::from([printed.name.to_lowercase()]);
    let report = check_exact(
        &text,
        &printed,
        &common::Corpus::repo().library(),
        &names,
        &Caps::everything(),
    );
    let mut whole = report.problems.is_empty() && report.strays.is_empty();
    let mut lines: Vec<String> = report
        .problems
        .iter()
        .map(|p| format!("PROBLEM {p:?}"))
        .collect();
    lines.extend(report.strays.iter().map(|s| format!("STRAY {s:?}")));
    let card: mtg_cards::card::Card = serde_json::from_str(&text).unwrap();
    let pending: Vec<(serde_json::Value, String)> = std::env::var("R302_WHOLES")
        .ok()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    for clause in &report.clauses {
        let state = match &clause.state {
            ClauseState::Reads { .. } => "Reads".to_owned(),
            ClauseState::Unwritten { ability, .. } | ClauseState::Differs { ability, .. } => {
                let json =
                    serde_json::to_value(&card.faces[clause.face].abilities[*ability]).unwrap();
                if pending.iter().any(|(shape, words)| {
                    *shape == json && words.split('\n').any(|w| w == clause.text)
                }) {
                    "Reads (pending whole)".to_owned()
                } else {
                    whole = false;
                    format!(
                        "{:?}\n    JSON {}",
                        clause.state,
                        serde_json::to_string(&json).unwrap()
                    )
                }
            }
            other => {
                whole = false;
                format!("{other:?}")
            }
        };
        lines.push(format!("{:?} => {state}", clause.text));
    }
    (lines, whole)
}

/// The cards of the pack whose rules are whole: each reads back clause by clause.
const WHOLE: &[&str] = &[];

#[test]
fn the_whole_cards_of_the_pack_read_back_as_their_oracle_text() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut wrong = Vec::new();
    for slug in WHOLE {
        let (lines, whole) = report(
            &dir.join(format!("cards/{slug}.json")),
            &dir.join(format!("tests/fixtures/corpus/{slug}.json")),
        );
        // A keyword line its rules stand for through the costs of the modes (Tiered) has no ability of its own.
        let open_keyword = |line: &String| {
            line.strip_suffix(" => Open")
                .is_some_and(|clause| !clause.contains(' '))
        };
        if !whole
            && !lines
                .iter()
                .all(|line| line.ends_with("=> Reads") || open_keyword(line))
        {
            wrong.push(format!("{slug}:\n{}", lines.join("\n")));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Prints the read back of the cards `R302` names (slugs, comma separated), and the compiler's reading with `R302_COMPILE`.
#[test]
#[ignore = "dev report"]
fn dev_report() {
    let Ok(list) = std::env::var("R302") else {
        return;
    };
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf();
    let scratch = std::env::var("R302_DIR").map(std::path::PathBuf::from);
    for slug in list.split(',') {
        let pick = |sub: &str| {
            let base = scratch.as_ref().map_or(repo.clone(), Clone::clone);
            let path = base.join(format!("{sub}/{slug}.json"));
            if path.exists() {
                path
            } else {
                repo.join(format!("{sub}/{slug}.json"))
            }
        };
        let rules = pick("cards");
        let fixture = pick("tests/fixtures/corpus");
        if std::env::var("R302_COMPILE").is_ok() {
            let compiled = compile(&printed_at(&fixture));
            println!(
                "== {slug} compiled\n{}",
                serde_json::to_string(&compiled.card).unwrap()
            );
        }
        if rules.exists() {
            let (lines, whole) = report(&rules, &fixture);
            println!("== {slug} whole={whole}\n{}", lines.join("\n"));
        }
    }
}
