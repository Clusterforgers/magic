//! Goal: every card of the r28-3 pack reads back as its printed text: the checker accepts its rules file whole, so a
//! `Full` card is `Verified` by proof and a `Partial` card leaves only its `Manual` clauses; the phrase atoms of the pack
//! read and write only their own words, an `{X}` is allowed only where a keyword cost holds one, a counted prevention reads
//! no target, and a counted payment counts only the payer's own cards.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;
use std::fmt::Write as _;

use mtg_cards::compile::compile;
use mtg_cards::load::{canonical, slug};
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::{Caps, PrintedCard, PrintedFace};
use mtg_import::scryfall::cards;
use rstest::rstest;

/// The cards of the pack.
const CARDS: &[&str] = &[
    "Ad Nauseam",
    "Amass the Components",
    "Angrath's Fury",
    "Avacyn's Judgment",
    "Basri's Aegis",
    "Basri, Devoted Paladin",
    "Beacon of Immortality",
    "Calamity of the Titans",
    "Candles' Glow",
    "Catch // Release",
    "Command of Unsummoning",
    "Command the Stage",
    "Contact Other Plane",
    "Crush of Tentacles",
    "Cut the Earthly Bond",
    "Deep Wood",
    "Dovin's Dismissal",
    "Enter the Infinite",
    "Errand of Duty",
    "Farideh's Fireball",
    "Feast of Dreams",
    "Fell Beast's Shriek",
    "Formation",
    "From Under the Floorboards",
    "Gideon's Battle Cry",
    "Gideon, the Oathsworn",
    "Gloomwidow's Feast",
    "Grip of Amnesia",
    "Heavy Fog",
    "Hezrou // Demonic Stench",
    "Hinder",
    "Hustle // Bustle",
    "Illuminate",
    "Jabari's Influence",
    "Jace's Ruse",
    "Jace, Arcane Strategist",
    "Laquatus's Disdain",
    "Lava Blister",
    "Liliana's Scorn",
    "Liliana, Death Mage",
    "March of Souls",
    "Misinformation",
    "Monastery Raid",
    "Monstrous Emergence",
    "Not of This World",
    "Outwit",
    "Overwhelming Encounter",
    "Perplex",
    "Pistus Strike",
    "Pull from Eternity",
    "Raphael's Technique",
    "Reciprocate",
    "Recruitment Drive",
    "Rise // Fall",
    "Rise from the Wreck",
    "Runic Repetition",
    "Scout for Survivors",
    "Second Guess",
    "Spider-Sense",
    "Starnheim Unleashed",
    "Temper",
    "Test of Faith",
    "The Crowd Goes Wild",
    "The Last Ronin's Technique",
    "Titan's Presence",
    "Unforge",
    "Venomous Vines",
    "Vraska's Stoneglare",
    "Vraska, Regal Gorgon",
    "Wake the Dead",
    "Winds of Change",
];

#[test]
fn every_r283_card_reads_back_as_its_text() {
    let library = common::Corpus::repo().library();
    let printed = common::printed();
    let names: BTreeSet<String> = printed
        .iter()
        .map(|card| card.name.to_lowercase())
        .collect();
    // `R283_ONLY` names the cards to check, joined by `|`, in place of the list.
    let only = std::env::var("R283_ONLY").unwrap_or_default();
    let wanted: Vec<&str> = match only.as_str() {
        "" => CARDS.to_vec(),
        only => only.split('|').collect(),
    };
    let mut failed = Vec::new();
    for name in wanted {
        let card = printed
            .iter()
            .find(|card| card.name == name)
            .unwrap_or_else(|| panic!("no fixture for {name}"));
        let path = common::cards_dir().join(format!("{}.json", slug(name)));
        let text = std::fs::read_to_string(&path).unwrap();
        let report = check_exact(&text, card, &library, &names, &Caps::everything());
        let plays = ["ACCEPTED Verified", "ACCEPTED Partial"].contains(&report.summary().as_str());
        if report.verdict() != Verdict::Accepted || !plays || std::env::var("R283_SHOW").is_ok() {
            failed.push(report.render(true));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n\n"));
}

/// A tool, not a check: `R283_PROBE=<dir of Scryfall objects>` compiles each card and checks the rules file of the
/// same slug in `R283_RULES=<dir>` (else the compiled rules), printing both.
#[test]
#[ignore = "a probe for writing the pack's cards"]
fn probe() {
    let Ok(dir) = std::env::var("R283_PROBE") else {
        return;
    };
    let rules = std::env::var("R283_RULES").unwrap_or_default();
    let filter = std::env::var("R283_ONLY").unwrap_or_default();
    let library = common::Corpus::repo().library();
    let mut names: BTreeSet<String> = common::printed()
        .iter()
        .map(|card| card.name.to_lowercase())
        .collect();
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    let mut out = String::new();
    for path in entries {
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        if !filter.is_empty() && !filter.split('|').any(|f| f == stem) {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let record = cards(format!("[{text}]").as_bytes())
            .next()
            .unwrap()
            .unwrap();
        let printed = common::printed_of(&record);
        names.insert(printed.name.to_lowercase());
        let compiled = compile(&printed);
        let compiled_text = compiled.card.as_ref().map(canonical).unwrap_or_default();
        let file = std::path::Path::new(&rules).join(format!("{stem}.json"));
        let (source, proposal) = match std::fs::read_to_string(&file) {
            Ok(own) => ("file", own),
            Err(_) => ("compiled", compiled_text.clone()),
        };
        let report = check_exact(&proposal, &printed, &library, &names, &Caps::everything());
        let _ = writeln!(out, "=== {} [{source}] {}", printed.name, report.summary());
        if std::env::var("R283_JSON").is_ok() || source == "compiled" {
            out.push_str(&compiled_text);
            out.push('\n');
        }
        out.push_str(&report.render(true));
        out.push('\n');
    }
    let target = std::env::var("R283_OUT").unwrap_or_else(|_| "/dev/stdout".into());
    std::fs::write(target, out).unwrap();
}

/// A printed instant of the given cost and text.
fn r283_instant(cost: &str, text: &str) -> PrintedCard {
    PrintedCard {
        layout: "normal".into(),
        oracle_id: "5a3d7ed4-7a50-4f22-9a3e-9b3f3a2a8d10".into(),
        name: "Test Spell".into(),
        keywords: vec![],
        faces: vec![PrintedFace {
            name: "Test Spell".into(),
            mana_cost: cost.into(),
            type_line: "Instant".into(),
            oracle_text: text.into(),
            ..PrintedFace::default()
        }],
    }
}

/// The verdict and support summary of a proposal for a printed instant.
fn r283_summary(cost: &str, text: &str, abilities: &str) -> String {
    let library = common::Corpus::repo().library();
    let names = BTreeSet::new();
    let card = r283_instant(cost, text);
    let json = format!(r#"{{"faces":[{{"abilities":[{abilities}]}}]}}"#);
    check_exact(&json, &card, &library, &names, &Caps::everything()).summary()
}

#[rstest]
#[case::cast_from_a_graveyard(
    "Counter target spell cast from a graveyard.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"CastFrom":"Graveyard"}]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#,
    "ACCEPTED Verified"
)]
#[case::cast_from_a_hand_is_other_words(
    "Counter target spell cast from a graveyard.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"CastFrom":"Hand"}]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#,
    "NOT ACCEPTED"
)]
#[case::the_second_spell_cast(
    "Counter target spell that's the second spell cast this turn.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"NthCastThisTurn":2}]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#,
    "ACCEPTED Verified"
)]
#[case::the_third_spell_is_other_words(
    "Counter target spell that's the second spell cast this turn.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"NthCastThisTurn":3}]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#,
    "NOT ACCEPTED"
)]
#[case::targets_a_player(
    "Counter target spell that targets a player.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"TargetsPlayer":"EachPlayer"}]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#,
    "ACCEPTED Verified"
)]
#[case::targets_you_is_not_a_player(
    "Counter target spell that targets a player.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"TargetsPlayer":"You"}]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#,
    "NOT ACCEPTED"
)]
#[case::dealt_damage_to_you(
    "Exile target creature that dealt damage to you this turn.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"DidThisTurn":{"event":"Damaged","to":"You"}}]}}],"effects":[{"Exile":{"what":{"Target":0}}}]}}"#,
    "ACCEPTED Verified"
)]
#[case::dealt_damage_by_you_is_other_words(
    "Exile target creature that dealt damage to you this turn.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"DidThisTurn":{"event":"Damaged","by":"You"}}]}}],"effects":[{"Exile":{"what":{"Target":0}}}]}}"#,
    "NOT ACCEPTED"
)]
#[case::target_enchanted_permanent(
    "Destroy target enchanted permanent.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"HasAttached":[{"Subtype":"Aura"}]}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#,
    "ACCEPTED Verified"
)]
#[case::equipped_is_not_enchanted(
    "Destroy target enchanted permanent.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"HasAttached":[{"Subtype":"Equipment"}]}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#,
    "NOT ACCEPTED"
)]
#[case::hand_card_on_the_bottom(
    "Draw three cards, then put a card from your hand on the bottom of your library.",
    r#"{"Spell":{"effects":[{"Draw":{"count":3}},{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"}]},"as":"back"}},{"Move":{"what":{"Bound":"back"},"to":{"Library":{"place":"Bottom"}}}}]}}"#,
    "ACCEPTED Verified"
)]
#[case::hand_card_on_top_is_other_words(
    "Draw three cards, then put a card from your hand on the bottom of your library.",
    r#"{"Spell":{"effects":[{"Draw":{"count":3}},{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"}]},"as":"back"}},{"Move":{"what":{"Bound":"back"},"to":{"Library":{"place":"Top"}}}}]}}"#,
    "NOT ACCEPTED"
)]
fn r283_the_phrase_atoms_read_back_only_as_their_own_words(
    #[case] text: &str,
    #[case] abilities: &str,
    #[case] wanted: &str,
) {
    let summary = r283_summary("{U}", text, abilities);
    match wanted.strip_prefix("NOT ") {
        Some(refused) => assert!(!summary.starts_with(refused), "{summary}"),
        None => assert!(summary.starts_with(wanted), "{summary}"),
    }
}

#[rstest]
#[case::madness_x_gives_the_spell_x("Madness {X}{B}", true)]
#[case::foretell_x_gives_the_spell_x("Foretell {X}{W}", true)]
#[case::freerunning_x_gives_the_spell_x("Freerunning {X}{R}", true)]
#[case::a_madness_cost_without_x_does_not("Madness {B}", false)]
#[case::a_flashback_x_does_not("Flashback {X}{B}", false)]
fn r283_x_is_allowed_only_where_a_keyword_cost_holds_one(
    #[case] keyword: &str,
    #[case] valid: bool,
) {
    let (name, cost) = keyword.split_once(' ').unwrap();
    let text = format!("{keyword}\nYou gain X life.");
    let abilities = format!(
        r#"{{"Keyword":{{"{name}":"{cost}"}}}},{{"Spell":{{"effects":[{{"GainLife":{{"amount":"X"}}}}]}}}}"#
    );
    let summary = r283_summary("{1}{B}", &text, &abilities);
    assert_eq!(!summary.starts_with("INVALID"), valid, "{summary}");
}

#[rstest]
#[case::the_damaged_one(
    r#"{"PutCounters":{"on":"Triggering","counter":"+1/+1","count":"Triggering"}}"#,
    true
)]
#[case::a_target(
    r#"{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":"Triggering"}}"#,
    false
)]
fn r283_a_counted_prevention_reads_no_target(#[case] effect: &str, #[case] valid: bool) {
    let text = "Prevent the next 3 damage that would be dealt to target creature this turn. For each 1 damage prevented this way, put a +1/+1 counter on that creature.";
    let abilities = format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"Shield":{{"when":{{"Damage":{{"to":{{"Target":0}}}}}},"by":{{"PreventCountedAnd":{{"most":3,"effects":[{effect}]}}}},"duration":"UntilEndOfTurn"}}}}]}}}}"#
    );
    let summary = r283_summary("{1}{W}", text, &abilities);
    assert_eq!(!summary.starts_with("INVALID"), valid, "{summary}");
}

#[rstest]
#[case::the_payers_own_hand(r#"{"OwnedBy":{"ControllerOf":{"Target":0}}}"#, true)]
#[case::the_casters_hand(r#"{"OwnedBy":"You"}"#, false)]
fn r283_a_counted_payment_counts_only_the_payers_own_cards(
    #[case] owner: &str,
    #[case] runs: bool,
) {
    let ability: mtg_cards::ability::Ability = serde_json::from_str(&format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":["Spell"]}}}}],"effects":[{{"Pay":{{"who":{{"ControllerOf":{{"Target":0}}}},"cost":[{{"Discard":{{"count":{{"Count":[{{"In":"Hand"}},{owner}]}}}}}}],"unpaid":[{{"Counter":{{"what":{{"Target":0}}}}}}]}}}}]}}}}"#
    ))
    .unwrap();
    let used = mtg_cards::validate::used_primitives(&ability);
    assert_eq!(used.contains("Effect.Pay.Counted"), runs, "{used:?}");
    assert_eq!(
        !used
            .iter()
            .any(|name| name.starts_with("Cost.Discard.Counted")),
        runs,
        "{used:?}"
    );
}
