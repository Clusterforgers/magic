//! Goal: every card the r61-04 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another zone, a limit or rider dropped, another cost) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r6104_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r6104_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r6104_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    check(
        text,
        card,
        &tokens,
        &r6104_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R6104_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r6104_probe() {
    if let Ok(json) = std::env::var("R6104_JSON") {
        for line in json.split('\n') {
            let ability: Ability = serde_json::from_str(line).unwrap();
            println!("-- {line}");
            for text in mtg_cards::compile::written_variants(&ability)
                .iter()
                .take(4)
            {
                println!("  written: {text}");
            }
        }
    }
    if let Ok(list) = std::env::var("R6104_COMPILE") {
        for name in list.split('|') {
            let card = common::printed()
                .into_iter()
                .find(|card| card.name == name)
                .unwrap();
            let compiled = mtg_cards::compile::compile(&card);
            println!("== {name}");
            for clause in &compiled.clauses {
                println!("  clause read={} {}", clause.read, clause.text);
            }
            if let Some(card) = compiled.card {
                for face in &card.faces {
                    for ability in &face.abilities {
                        println!("  {}", serde_json::to_string(ability).unwrap());
                    }
                }
            }
        }
    }
    let Ok(list) = std::env::var("R6104_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r6104_report(name, &r6104_file(name));
        println!("== {name}: {:?}", report.verdict());
        for problem in &report.problems {
            println!("  problem: {problem:?}");
        }
        for clause in &report.clauses {
            println!("  [{}] {} => {:?}", clause.face, clause.text, clause.state);
        }
        for stray in &report.strays {
            println!("  stray: {stray:?}");
        }
        let card: mtg_cards::card::Card = serde_json::from_str(&r6104_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Moonlit Meditation")]
#[case("Mirrormind Crown")]
#[case("Esix, Fractal Bloom")]
#[case("Jinnie Fay, Jetmir's Second")]
#[case("Hulking Metamorph")]
#[case("Moritte of the Frost")]
#[case("Ink-Treader Nephilim")]
#[case("Radiate")]
#[case("Bill Potts")]
#[case("Shifting Woodland")]
#[case("Mindlink Mech")]
fn r6104_reads_back_whole(#[case] name: &str) {
    let report = r6104_report(name, &r6104_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::meditation_every_time("Moonlit Meditation", r#""op": "Eq""#, r#""op": "Ge""#)]
#[case::meditation_copies_itself("Moonlit Meditation", r#""of": "Attached""#, r#""of": "This""#)]
#[case::meditation_one_token("Moonlit Meditation", r#""count": "Triggering""#, r#""count": 1"#)]
#[case::crown_unattached_too("Mirrormind Crown", r#""op": "Ge""#, r#""op": "Le""#)]
#[case::esix_any_turn("Esix, Fractal Bloom", r#""YourTurn","#, "")]
#[case::esix_itself_too("Esix, Fractal Bloom", r#""Other""#, r#""Token""#)]
#[case::jinnie_dog_haste("Jinnie Fay, Jetmir's Second", r#""Vigilance""#, r#""Haste""#)]
#[case::metamorph_any_controller(
    "Hulking Metamorph",
    r#"{ "ControlledBy": "You" }"#,
    r#"{ "ControlledBy": "EachPlayer" }"#
)]
#[case::metamorph_copied_size("Hulking Metamorph", r#""of": "This""#, r#""of": "Triggering""#)]
#[case::moritte_changeling_always(
    "Moritte of the Frost",
    r#""of": [{ "Type": "Creature" }], "except""#,
    r#""of": [], "except""#
)]
#[case::moritte_one_counter("Moritte of the Frost", r#""count": 2"#, r#""count": 1"#)]
#[case::nephilim_any_target(
    "Ink-Treader Nephilim",
    r#"{ "R6104TargetsOnly": ["This"] }"#,
    r#"{ "Targeting": ["This"] }"#
)]
#[case::nephilim_artifacts(
    "Ink-Treader Nephilim",
    r#""each_target": [{ "Type": "Creature" }]"#,
    r#""each_target": [{ "Type": "Artifact" }]"#
)]
#[case::radiate_no_players("Radiate", r#""each_player": true"#, r#""each_player": false"#)]
#[case::bill_every_time("Bill Potts", r#""limit": "OncePerTurn","#, "")]
#[case::bill_any_caster("Bill Potts", r#""by": "You""#, r#""by": "EachPlayer""#)]
#[case::woodland_three_types("Shifting Woodland", r#""right": 4"#, r#""right": 3"#)]
#[case::woodland_forever("Shifting Woodland", r#""UntilEndOfTurn""#, r#""Forever""#)]
#[case::mech_every_crew("Mindlink Mech", r#""nth": 1"#, r#""nth": 2"#)]
#[case::mech_legendary_too("Mindlink Mech", r#"{ "Not": { "Supertype": "Legendary" } },"#, "")]
fn r6104_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r6104_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r6104_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
