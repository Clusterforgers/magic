//! Goal: every card the r113-04 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss of each (another type, zone, player, count or duration), replaced wherever it occurs, does not.
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
#[case("Uvilda, Dean of Perfection // Nassari, Dean of Expression")]
#[case("Spellweaver Volute")]
#[case("Vedalken Squirrel-Whacker")]
#[case("Vault 21: House Gambit")]
#[case("Cosima, God of the Voyage // The Omenkeel")]
#[case("Merseine")]
#[case("Blade of the Swarm")]
#[case("The Pandorica")]
#[case("Bell Borca, Spectral Sergeant")]
#[case("Volo, Itinerant Scholar")]
#[case("Casey & Raph, Hotheads")]
#[case("Soundwave, Sonic Spy // Soundwave, Superior Captain")]
#[case("Zabaz, the Glimmerwasp")]
fn r113_04_reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &compact(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::your_library(
    "Uvilda, Dean of Perfection // Nassari, Dean of Expression",
    r#""who":"Opponents""#,
    r#""who":"You""#
)]
#[case::no_spend(
    "Uvilda, Dean of Perfection // Nassari, Dean of Expression",
    r#","spend_as":"AnyColor""#,
    ""
)]
#[case::forever(
    "Uvilda, Dean of Perfection // Nassari, Dean of Expression",
    r#""UntilEndOfTurn""#,
    r#""Forever""#
)]
#[case::instant(
    "Spellweaver Volute",
    r#"[{"Type":"Sorcery"}]"#,
    r#"[{"Type":"Instant"}]"#
)]
#[case::creature_card(
    "Spellweaver Volute",
    r#"{"Enchant":[{"Type":"Instant"}"#,
    r#"{"Enchant":[{"Type":"Creature"}"#
)]
#[case::paid("Spellweaver Volute", r#""free":true,"#, "")]
#[case::no_exile("Spellweaver Volute", r#"{"Exile":{"what":"Attached"}},"#, "")]
#[case::eight_sided(
    "Vedalken Squirrel-Whacker",
    r#""sides":6,"slot""#,
    r#""sides":8,"slot""#
)]
#[case::three_dice("Vedalken Squirrel-Whacker", r#""dice":2"#, r#""dice":3"#)]
#[case::both_first("Vedalken Squirrel-Whacker", r#""index":1"#, r#""index":0"#)]
#[case::six("Vault 21: House Gambit", r#""max":5"#, r#""max":6"#)]
#[case::power("Vault 21: House Gambit", r#""ManaValue""#, r#""Power""#)]
#[case::any_card(
    "Cosima, God of the Voyage // The Omenkeel",
    r#"[{"Type":"Land"}]"#,
    "[]"
)]
#[case::cast_only("Cosima, God of the Voyage // The Omenkeel", r#","play":true"#, "")]
#[case::its_controller("Merseine", r#""R113AttachedController""#, r#""Controller""#)]
#[case::any_exiled("Blade of the Swarm", r#",{"R113HasWay":"Warp"}"#, "")]
#[case::flashback("Blade of the Swarm", r#""Warp""#, r#""Flashback""#)]
#[case::held_forever("The Pandorica", r#""WhileSourceTapped""#, r#""Forever""#)]
#[case::no_phase_in("The Pandorica", r#"{"R113PhaseIn":{"what":{"Target":0}}}"#, "")]
#[case::toughness("Bell Borca, Spectral Sergeant", r#""SetPower""#, r#""SetToughness""#)]
#[case::artifact_spell(
    "Volo, Itinerant Scholar",
    r#"[{"Type":"Creature"}]"#,
    r#"[{"Type":"Artifact"}]"#
)]
#[case::same_player("Casey & Raph, Hotheads", r#","different_players":true"#, "")]
#[case::your_end_step(
    "Casey & Raph, Hotheads",
    r#""R113UntilTheirNextEndStep""#,
    r#""UntilYourNextEndStep""#
)]
#[case::paid("Casey & Raph, Hotheads", r#""free":true,"#, "")]
#[case::odd_twice(
    "Soundwave, Sonic Spy // Soundwave, Superior Captain",
    r#""odd":false"#,
    r#""odd":true"#
)]
#[case::nontoken(
    "Soundwave, Sonic Spy // Soundwave, Superior Captain",
    r#","Token""#,
    ""
)]
#[case::any_value(
    "Soundwave, Sonic Spy // Soundwave, Superior Captain",
    r#"{"Compare":{"op":"Eq","stat":"ManaValue","to":"Triggering"}},"#,
    ""
)]
#[case::any_trigger("Zabaz, the Glimmerwasp", r#","r113_keyword":"Modular""#, "")]
#[case::riot("Zabaz, the Glimmerwasp", r#""Modular""#, r#""Riot""#)]
#[case::any_journal("Volo, Itinerant Scholar", r#",{"Named":"Volo's Journal"}"#, "")]
fn r113_04_a_near_miss_does_not_read_back(
    #[case] name: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let rules = compact(name);
    assert!(rules.contains(from), "{name}: {from}");
    let report = report_of(name, &rules.replace(from, to));
    assert_ne!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {from} -> {to}"
    );
}
