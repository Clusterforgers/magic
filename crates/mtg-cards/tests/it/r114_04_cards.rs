//! Goal: every card the r114-04 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss of each (another sticker kind, zone, ticket cost, die or count), replaced wherever it occurs, does not.
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
#[case("Clandestine Chameleon")]
#[case("Pin Collection")]
#[case("Clay Golem")]
#[case("Wicker Picker")]
fn r114_04_reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &compact(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::itself_too("Clandestine Chameleon", r#""Other","#, "")]
#[case::exile("Clandestine Chameleon", r#"{"In":"Graveyard"}"#, r#"{"In":"Exile"}"#)]
#[case::name_sticker("Pin Collection", r#""kind":"Ability""#, r#""kind":"Name""#)]
#[case::two_or_less(
    "Pin Collection",
    r#""r114_free_at_most":"X""#,
    r#""r114_free_at_most":2"#
)]
#[case::paid("Pin Collection", r#","r114_free_at_most":"X""#, "")]
#[case::on_the_creature(
    "Pin Collection",
    r#"{"R114StickerAbilitiesOf":"This"}"#,
    r#"{"R114StickerAbilitiesOf":"Attached"}"#
)]
#[case::artifact_spells(
    "Wicker Picker",
    r#"[{"Type":"Creature"}]"#,
    r#"[{"Type":"Artifact"}]"#
)]
#[case::two_mana("Wicker Picker", r#""cost":"{1}""#, r#""cost":"{2}""#)]
#[case::six_sided("Clay Golem", r#"{"R114RollDie":8}"#, r#"{"R114RollDie":6}"#)]
#[case::two_counters("Clay Golem", r#""count":"X""#, r#""count":2"#)]
fn r114_04_a_near_miss_does_not_read_back(
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
