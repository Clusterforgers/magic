//! Goal: every distinct mana cost string in Scryfall's bulk data is either read into a cost with the mana value and colors Scryfall gives it, or refused by name for a symbol the engine leaves out; none is skipped, misread or half read.
//!
//! The strings are the distinct `mana_cost` values of the oracle-cards file of 2026-10-17
//! (`fixtures/scryfall_mana_costs.tsv`); `real_data.rs` repeats the check on a fresh download.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::collections::{BTreeMap, BTreeSet};

use mtg_core::color::{Color, ColorSet};
use mtg_core::mana::{ManaCost, ManaSymbol};
use rstest::rstest;
use support::{OUT_OF_SCOPE, Verdict, symbols_of, verdict};

const FIXTURE: &str = include_str!("fixtures/scryfall_mana_costs.tsv");

struct Row {
    kind: &'static str,
    cost: &'static str,
    /// Scryfall's `cmc` and `colors` for a single-faced card; `None` colours when only devoid cards carry the cost.
    expected: Option<(f64, Option<ColorSet>)>,
}

fn rows() -> Vec<Row> {
    FIXTURE
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let mut parts = line.split('\t');
            let kind = parts.next().unwrap();
            let cost = parts.next().unwrap();
            let expected = parts.next().map(|cmc| {
                let colors = parts.next().unwrap();
                let set = match colors {
                    "?" => None,
                    "-" => Some(ColorSet::EMPTY),
                    letters => Some(letters.chars().filter_map(Color::from_symbol).collect()),
                };
                (cmc.parse().unwrap(), set)
            });
            Row {
                kind,
                cost,
                expected,
            }
        })
        .collect()
}

#[test]
fn the_fixture_is_the_size_of_the_real_data() {
    let all = rows();
    let count = |kind: &str| all.iter().filter(|r| r.kind == kind).count();
    assert_eq!(
        (count("card"), count("face"), count("faces")),
        (864, 211, 297)
    );
}

#[test]
fn every_single_faced_cost_reads_with_the_value_and_colors_scryfall_gives_it() {
    let mut parsed = 0;
    let mut refused = BTreeSet::new();
    for row in rows().into_iter().filter(|r| r.kind == "card") {
        let (cmc, colors) = row.expected.unwrap();
        match verdict(row.cost) {
            Verdict::Parsed(costs) => {
                parsed += 1;
                let [cost] = costs.as_slice() else {
                    panic!("{}: one cost expected", row.cost);
                };
                assert_eq!(
                    f64::from(cost.mana_value()),
                    cmc,
                    "{}: mana value",
                    row.cost
                );
                if let Some(colors) = colors {
                    assert_eq!(cost.colors(), colors, "{}: colors", row.cost);
                }
            }
            Verdict::Refused(symbol) => {
                refused.insert(symbol);
            }
        }
    }
    // 106 of the 864 use X or another symbol left out; every other cost is read, and agrees with Scryfall.
    assert_eq!(parsed, 758);
    // Pinned, the first symbol refused in each cost: a new kind of symbol in the data shows up here before anything is dropped.
    assert_eq!(
        refused.into_iter().collect::<Vec<_>>(),
        ["D", "HW", "S", "X"]
    );
}

#[test]
fn every_face_cost_and_every_card_level_face_list_reads_or_is_refused_by_name() {
    let mut faces_read = 0;
    for row in rows().into_iter().filter(|r| r.kind != "card") {
        if let Verdict::Parsed(costs) = verdict(row.cost) {
            faces_read += costs.len();
            let faces = row.cost.split(" // ").count();
            assert_eq!(costs.len(), faces, "{}: one cost per face", row.cost);
        }
    }
    assert!(faces_read >= 600, "only {faces_read} face costs were read");
}

#[test]
fn a_split_card_level_text_is_not_one_cost() {
    // `parse` takes one cost; the card-level text of a split card has to go through `parse_faces`.
    assert!(ManaCost::parse("{2}{B} // {B}").is_err());
    assert_eq!(ManaCost::parse_faces("{2}{B} // {B}").unwrap().len(), 2);
}

#[test]
fn every_symbol_in_the_data_is_one_the_engine_models_or_leaves_out_on_purpose() {
    let mut modelled = BTreeMap::<&str, usize>::new();
    let mut left_out = BTreeSet::new();
    for row in rows() {
        for symbol in symbols_of(row.cost) {
            match ManaCost::parse(&format!("{{{symbol}}}")) {
                Ok(_) => *modelled.entry(symbol).or_default() += 1,
                Err(_) => {
                    left_out.insert(symbol);
                }
            }
        }
    }
    assert_eq!(left_out.into_iter().collect::<Vec<_>>(), OUT_OF_SCOPE);
    // Every form of symbol CR 107.4 describes, as Scryfall writes it, is in the data and read.
    let forms = [
        ("{W}", "colored"),
        ("{C}", "colorless"),
        ("{16}", "generic"),
        ("{2/W}", "monocolored hybrid"),
        ("{2/G}", "monocolored hybrid"),
        ("{W/U}", "hybrid"),
        ("{G/W}", "hybrid"),
        ("{R/G}", "hybrid"),
        ("{W/P}", "Phyrexian"),
        ("{C/P}", "colorless Phyrexian"),
        ("{G/U/P}", "hybrid Phyrexian"),
        ("{R/W/P}", "hybrid Phyrexian"),
        ("{C/W}", "colorless hybrid"),
        ("{C/G}", "colorless hybrid"),
    ];
    for (symbol, kind) in forms {
        let inner = symbol.trim_matches(|c| c == '{' || c == '}');
        assert!(
            modelled.contains_key(inner),
            "no {kind} symbol {symbol} in the data"
        );
    }
    assert_eq!(modelled.len(), 61 - OUT_OF_SCOPE.len());
}

#[test]
fn what_is_read_loses_no_symbol_and_prints_back_to_the_same_cost() {
    for row in rows() {
        let Verdict::Parsed(costs) = verdict(row.cost) else {
            continue;
        };
        for (text, cost) in row.cost.split(" // ").zip(costs) {
            let shown = cost.to_string();
            assert_eq!(ManaCost::parse(&shown).as_ref(), Ok(&cost), "{text}");
            // Numbers add up to the generic part; every other symbol is one symbol of the cost.
            let written = symbols_of(text);
            let numbers: u32 = written.iter().filter_map(|s| s.parse::<u32>().ok()).sum();
            let others = written.iter().filter(|s| s.parse::<u32>().is_err()).count();
            let kept = cost
                .symbols()
                .iter()
                .filter(|s| !matches!(s, ManaSymbol::Generic(_)))
                .count();
            assert_eq!(
                (cost.generic(), kept),
                (numbers, others),
                "{text} -> {shown}"
            );
        }
    }
}

#[rstest]
#[case::boros_reckoner("{R/W}{R/W}{R/W}", 4)]
#[case::kitchen_finks("{1}{G/W}{G/W}", 3)]
#[case::dismember("{1}{B/P}{B/P}", 3)]
#[case::reaper_king("{2/W}{2/U}{2/B}{2/R}{2/G}", 32)]
#[case::flame_javelin("{R/P}{R/P}{R/P}", 4)]
fn the_choices_in_famous_costs_are_few(#[case] text: &str, #[case] choices: usize) {
    assert_eq!(ManaCost::parse(text).unwrap().resolutions().len(), choices);
}

#[test]
fn no_real_cost_has_a_choice_count_that_could_slow_a_priority_decision() {
    let most = rows()
        .into_iter()
        .filter_map(|row| match verdict(row.cost) {
            Verdict::Parsed(costs) => Some(costs),
            Verdict::Refused(_) => None,
        })
        .flatten()
        .map(|cost| cost.resolutions().len())
        .max();
    assert_eq!(most, Some(32));
}

#[test]
fn symbols_have_the_value_the_rules_give_them() {
    let value = |text: &str| ManaCost::parse(text).unwrap().mana_value();
    assert_eq!(value("{2/W}"), 2);
    assert_eq!(value("{W/P}"), 1);
    assert_eq!(value("{G/U/P}"), 1);
    assert_eq!(value("{C/W}"), 1);
    assert_eq!(value("{C/P}"), 1);
    assert_eq!(value("{1000000}"), 1_000_000);
    assert!(matches!(
        ManaCost::parse("{2/W}").unwrap().symbols(),
        [ManaSymbol::MonoHybrid(Color::White)]
    ));
}
