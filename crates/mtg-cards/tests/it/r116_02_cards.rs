//! Goal: every card the r116-02 pack made fully automatic reads back as its printed text clause by clause, and a near miss
//! of each (another number, filter or trigger) does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn report_of(name: &str, text: &str) -> mtg_cards::proposal::Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    check(text, card, &tokens, &names(&cards), &Caps::everything())
}

/// The rules file of a corpus card as compact JSON.
fn compact(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    let text = std::fs::read_to_string(&path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    serde_json::to_string(&value).unwrap()
}

#[rstest]
#[case("Andromedan Automatons")]
#[case("Anthropological Mission")]
#[case("Balance of Terror")]
#[case("Balefang the Unslayable")]
#[case("Blastoneuron")]
#[case("Blood-Sport Champion")]
#[case("Brad Boimler, Eager Ensign")]
#[case("Cantankerous Captain")]
#[case("Captain Janeway, Endgame")]
#[case("Captain Michael Burnham")]
#[case("Chief Kyle, Transporter Tech")]
#[case("Commander Raffi Musiker")]
#[case("Conquest Mission")]
#[case("Constable Odo")]
#[case("Culber and Stamets, Star-Crossed")]
#[case("Curzon Dax, Bound by Blood")]
#[case("David and Carol Marcus, Creators")]
#[case("D'Ghor Bloodletter")]
#[case("Dr. Leonard \"Bones\" McCoy")]
#[case("Ensign Harry Kim, Operations")]
#[case("Erica Ortegas, Plucky Pilot")]
#[case("Exploratory Mission")]
#[case("Federation Outpost")]
#[case("Gagh Slinger")]
#[case("Georgiou, Terran Emperor")]
#[case("Ghost of Illyria")]
#[case("Giant Space Amoeba")]
#[case("Gnarling Targ")]
#[case("Gul Dukat, Colonial Prefect")]
#[case("Hologram Janeway")]
#[case("Hologram Stalker")]
#[case("Indelicate Healer")]
#[case("Jake Sisko, War Correspondent")]
#[case("Janice Rand, Essential Aide")]
#[case("Kang, Suspicious Captain")]
#[case("Kes, Evolving Telepath")]
#[case("Khan, Despot by Design")]
#[case("Kirk, Commanding Officer")]
#[case("Kor, Cause of Vengeance")]
#[case("Lieutenant Hikaru Sulu")]
#[case("Lore, Positronic Provocateur")]
#[case("L'Rell, Fierce Visionary")]
#[case("Ni'Var Science Academy")]
#[case("Q Continuum")]
#[case("Quark's Bar")]
#[case("Sins of the Father")]
#[case("Spock, Always Your Friend")]
#[case("Stable Wormhole")]
#[case("Starfleet Security")]
#[case("Subcommander T'Pol")]
#[case("Sylvia Tilly, Joy of Discovery")]
#[case("Targ Wrangler")]
#[case("Temporal Conqueror")]
#[case("The Doctor, EMH")]
#[case("The Duras Sisters")]
#[case("The Inner Light")]
#[case("The Measure of a Man")]
#[case("The Naked Time")]
#[case("The Unicomplex")]
#[case("The Wrath of Khan")]
#[case("Those Old Scientists")]
#[case("Tribble Cluster")]
#[case("Troi, Winging It")]
#[case("Type-2 Phaser")]
#[case("Una Chin-Riley, Number One")]
#[case("Vash, Freelance Archaeologist")]
#[case("Vengeance Mission")]
#[case("Wesley Crusher, Starfleet Cadet")]
#[case("Will Riker, First in the Field")]
#[case("Worf, Honor-Bound Warrior")]
#[case("Worf, Still Not Merry")]
fn r116_02_reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &compact(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::any_creature("Curzon Dax, Bound by Blood", r#"{"Subtype":"Klingon"},"#, "")]
#[case::two_damage("Gagh Slinger", r#""amount":1"#, r#""amount":2"#)]
#[case::any_turn("Starfleet Security", r#""if":"YourTurn","#, "")]
#[case::seven_power("Hologram Janeway", r#""right":8"#, r#""right":7"#)]
#[case::tapped_land(
    "Federation Outpost",
    r#""selector":["Untapped"]"#,
    r#""selector":["Tapped"]"#
)]
#[case::one_life("Georgiou, Terran Emperor", r#""right":0"#, r#""right":1"#)]
#[case::four_quests("Anthropological Mission", r#""right":3"#, r#""right":4"#)]
#[case::plus_two("Brad Boimler, Eager Ensign", r#""Add":1"#, r#""Add":2"#)]
#[case::goaded_a_turn(
    "Kang, Suspicious Captain",
    r#""duration":"WhileYouControlSource""#,
    r#""duration":"UntilYourNextTurn""#
)]
#[case::three_lands(
    "Will Riker, First in the Field",
    r#""count":2,"as":"lands""#,
    r#""count":3,"as":"lands""#
)]
#[case::smaller("Ensign Harry Kim, Operations", r#""op":"Gt""#, r#""op":"Lt""#)]
fn r116_02_a_near_miss_does_not_read_back(
    #[case] name: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let text = compact(name);
    assert!(text.contains(from), "{name} holds {from}");
    let report = report_of(name, &text.replace(from, to));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}

/// Prints the compiler's reading of the cards of the newest Oracle Cards download that `MTG_PROBE_FILE` names, one
/// name a line (a tool for writing rules, run by hand).
#[test]
#[ignore = "a probe run by hand"]
fn r116_02_probe() {
    let names = std::fs::read_to_string(std::env::var("MTG_PROBE_FILE").unwrap()).unwrap();
    let names: Vec<&str> = names.lines().collect();
    let file = common::find_oracle_cards().unwrap();
    for card in common::printed_of_download(&file)
        .iter()
        .filter(|card| names.contains(&card.name.as_str()))
    {
        let compiled = mtg_cards::compile::compile(card);
        println!("=== {}", card.name);
        if std::env::var_os("MTG_PROBE_JSON").is_some() {
            println!("{}", mtg_cards::load::canonical(&compiled.card));
        }
        if let Some(dir) = std::env::var_os("MTG_PROBE_DUMP") {
            let path = std::path::Path::new(&dir)
                .join(format!("{}.json", mtg_cards::load::slug(&card.name)));
            std::fs::write(path, mtg_cards::load::canonical(&compiled.card)).unwrap();
        }
        for clause in &compiled.clauses {
            println!("{} {:?} {}", clause.read, clause.kind, clause.text);
        }
    }
}

/// Prints the compiler's reading of each `name|type line|text` line of `MTG_PROBE_TEXT` (" // " joins lines; a tool for
/// writing readers, run by hand).
#[test]
#[ignore = "a probe run by hand"]
fn r116_02_probe_text() {
    let lines = std::fs::read_to_string(std::env::var("MTG_PROBE_TEXT").unwrap()).unwrap();
    for line in lines.lines().filter(|line| !line.trim().is_empty()) {
        let mut parts = line.splitn(3, '|');
        let (name, type_line, text) = (
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap(),
        );
        let card = mtg_cards::support::PrintedCard {
            oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
            name: name.into(),
            faces: vec![mtg_cards::support::PrintedFace {
                name: name.into(),
                type_line: type_line.into(),
                oracle_text: text.replace(" // ", "\n"),
                ..Default::default()
            }],
            ..Default::default()
        };
        let compiled = mtg_cards::compile::compile(&card);
        println!("=== {name}");
        if std::env::var_os("MTG_PROBE_JSON").is_some() {
            for face in compiled.card.iter().flat_map(|card| &card.faces) {
                for ability in &face.abilities {
                    println!("J {}", serde_json::to_string(ability).unwrap());
                }
            }
        }
        for clause in &compiled.clauses {
            println!("{} {:?} {}", clause.read, clause.kind, clause.text);
        }
    }
}

/// Checks each `<slug>.json` of the directory `MTG_PROBE_DIR` against the card of the newest Oracle Cards download
/// with that slug, printing the verdict and every clause not read back; an accepted file's completed rules go to
/// `<slug>.out` (a tool for writing rules, run by hand).
#[test]
#[ignore = "a probe run by hand"]
fn r116_02_probe_check() {
    let dir = std::path::PathBuf::from(std::env::var("MTG_PROBE_DIR").unwrap());
    let file = common::find_oracle_cards().unwrap();
    let cards = common::printed_of_download(&file);
    let tokens = mtg_cards::load::Library::from_sources(&common::token_sources()).unwrap();
    let mut names: std::collections::BTreeSet<String> = common::printed()
        .iter()
        .map(|card| card.name.to_lowercase())
        .collect();
    names.extend(common::draft_names());
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    for path in paths
        .iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
    {
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let Some(card) = cards
            .iter()
            .find(|card| mtg_cards::load::slug(&card.name) == stem)
        else {
            println!("=== {stem}: no such card");
            continue;
        };
        names.insert(card.name.to_lowercase());
        let text = std::fs::read_to_string(path).unwrap();
        let report = mtg_cards::proposal::check(
            &text,
            card,
            &tokens,
            &names,
            &mtg_cards::Caps::everything(),
        );
        println!("=== {stem}: {:?}", report.verdict());
        for problem in &report.problems {
            println!("  problem {problem:?}");
        }
        for clause in &report.clauses {
            if !matches!(clause.state, mtg_cards::proposal::ClauseState::Reads { .. }) {
                println!("  {:?}\n    {}", clause.state, clause.text);
            }
        }
        for stray in &report.strays {
            println!("  stray {stray:?}");
        }
        if let Some(rules) = &report.rules
            && report.verdict() == mtg_cards::proposal::Verdict::Accepted
        {
            std::fs::write(
                dir.join(format!("{stem}.out")),
                mtg_cards::load::canonical(rules),
            )
            .unwrap();
        }
    }
}
