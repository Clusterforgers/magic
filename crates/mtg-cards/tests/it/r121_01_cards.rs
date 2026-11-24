//! Goal: every card the r121-01 pack made fully automatic reads back as its printed text clause by clause, and a near miss
//! of each (another player, mana, duration, zone or count) does not.
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
#[case("Borg Cube")]
#[case("The Best of Both Worlds")]
#[case("Colossal Clone Master")]
#[case("Dominion Saboteur")]
#[case("Gift of Perfection")]
#[case("Nog, Ambitious Cadet")]
#[case("T'Pring, Never Parted")]
#[case("The Guardian of Forever")]
#[case("The Madcap Jester")]
#[case("Tresserhorn's Lord, Returned")]
#[case("Zero, Incorporeal Sensate")]
fn r121_01_reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &compact(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?} {:?}",
        report.clauses,
        report.problems
    );
}

#[rstest]
#[case::an_opponent_caused_it("The Madcap Jester", r#""by":"You""#, r#""by":"Opponents""#)]
#[case::any_color("The Madcap Jester", r#""AnyType""#, r#""AnyColor""#)]
#[case::this_turn(
    "The Madcap Jester",
    r#""UntilEndOfYourNextTurn""#,
    r#""UntilEndOfTurn""#
)]
#[case::into_a_graveyard("The Madcap Jester", r#""to":"Exile""#, r#""to":"Graveyard""#)]
#[case::assimilated_anyway("Borg Cube", r#"{"Not":"Performed"}"#, r#""Performed""#)]
#[case::a_target_you_must_take("Borg Cube", r#""min":0"#, r#""min":1"#)]
#[case::not_remembered(
    "The Best of Both Worlds",
    r#""slot":"assimilated""#,
    r#""slot":"exiled""#
)]
#[case::any_assimilated(
    "The Best of Both Worlds",
    r#"{"Subtype":"Borg"}"#,
    r#"{"Subtype":"Elf"}"#
)]
#[case::from_exile(
    "The Guardian of Forever",
    r#""zone":"Graveyard""#,
    r#""zone":"Exile""#
)]
#[case::a_token_too("The Guardian of Forever", r#"{"Not":"Token"},"#, "")]
#[case::no_counters(
    "Dominion Saboteur",
    r#""r121_copied_counters":true"#,
    r#""tapped":true"#
)]
#[case::a_fight_not_needed(
    "T'Pring, Never Parted",
    r#""condition":"Performed""#,
    r#""condition":"YourTurn""#
)]
#[case::one_of_them("T'Pring, Never Parted", r#",{"Is":{"Target":1}}"#, "")]
#[case::once_only("T'Pring, Never Parted", r#""once":false"#, r#""once":true"#)]
#[case::not_defining("Colossal Clone Master", r#""defining":true"#, r#""defining":false"#)]
#[case::you_draw(
    "Tresserhorn's Lord, Returned",
    r#""who":{"Target":0}"#,
    r#""who":"You""#
)]
#[case::two_life("Tresserhorn's Lord, Returned", r#"{"PayLife":3}"#, r#"{"PayLife":2}"#)]
#[case::three_more(
    "Zero, Incorporeal Sensate",
    r#""additional":"{2}""#,
    r#""additional":"{3}""#
)]
#[case::you_play_it(
    "Zero, Incorporeal Sensate",
    r#"{"OwnerOf":{"Bound":"exiled"}}"#,
    r#""You""#
)]
#[case::any_scout(
    "Nog, Ambitious Cadet",
    r#"{"Subtype":"Rogue"}"#,
    r#"{"Subtype":"Scout"}"#
)]
#[case::no_extra_land(
    "Nog, Ambitious Cadet",
    r#",{"GrantAbility":{"Rule":{"rule":{"AdditionalLandPlays":{}}}}}"#,
    ""
)]
#[case::put_on_top("Gift of Perfection", r#""place":"Bottom""#, r#""place":"Top""#)]
#[case::still_legendary(
    "Dominion Saboteur",
    r#""except":[{"RemoveSupertype":"Legendary"}],"#,
    ""
)]
fn r121_01_a_near_miss_does_not_read_back(
    #[case] name: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let text = compact(name);
    assert!(text.contains(from), "{name} holds {from}");
    let report = report_of(name, &text.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
