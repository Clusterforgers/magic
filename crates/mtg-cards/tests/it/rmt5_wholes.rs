//! Goal: the rmt5 cards read back whole as their printed text, and the same ability with one count, color, event, kind or verb changed never reads back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;
use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::{PrintedCard, Support};
use rstest::rstest;

use common::{cards_dir, printed, token_sources};

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

/// The verdict and state of `rules` checked against the corpus card `name`.
fn judged(name: &str, rules: &str) -> (Verdict, Option<Support>) {
    let cards = printed();
    let tokens = Library::from_sources(&token_sources()).unwrap();
    let card = cards.iter().find(|card| card.name == name).unwrap();
    let report = check(rules, card, &tokens, &names(&cards), &Caps::everything());
    (
        report.verdict(),
        report.support.map(|record| record.support),
    )
}

/// The corpus file of the card as compact JSON.
fn compact(name: &str) -> String {
    let text = std::fs::read_to_string(cards_dir().join(format!("{}.json", slug(name)))).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value.to_string()
}

#[rstest]
#[case::flare_size(
    "Flare of Faith",
    r#""power":3,"toughness":3"#,
    r#""power":4,"toughness":4"#
)]
#[case::brimstone_self_damage(
    "Fire and Brimstone",
    r#""amount":4,"to":"You""#,
    r#""amount":3,"to":"You""#
)]
#[case::druid_event("Fyndhorn Druid", r#""event":"Blocked""#, r#""event":"Attacked""#)]
#[case::demolition_count("Gleeful Demolition", r#""count":3"#, r#""count":2"#)]
#[case::burnout_color("Burnout", r#""Color":"Blue""#, r#""Color":"Red""#)]
#[case::blaze_threshold("Searing Blaze", r#""right":1"#, r#""right":2"#)]
#[case::instability_verb("Tectonic Instability", r#"{"Tap":"#, r#"{"Untap":"#)]
#[case::spoils_kind("Spoils of Evil", r#""Type":"Artifact""#, r#""Type":"Enchantment""#)]
#[case::akuba_life("Wicked Akuba", r#""amount":1"#, r#""amount":2"#)]
#[case::hex_step("Artificer's Hex", r#""step":"Upkeep""#, r#""step":"End""#)]
fn rmt5_a_whole_reads_back_and_a_near_miss_does_not(
    #[case] name: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let rules = compact(name);
    assert_eq!(
        judged(name, &rules),
        (Verdict::Accepted, Some(Support::Verified)),
        "{name}"
    );
    let changed = rules.replace(from, to);
    assert_ne!(changed, rules, "{name}: the mutation applies");
    assert_ne!(
        judged(name, &changed).0,
        Verdict::Accepted,
        "{name}: {to} must not read back"
    );
}

#[rstest]
#[case::snack("Midnight Snack")]
#[case::entmoot("Assemble the Entmoot")]
#[case::tivash("Tivash, Gloom Summoner")]
#[case::sorin("Sorin of House Markov // Sorin, Ravenous Neonate")]
fn rmt5_the_amount_of_life_gained_is_the_total_not_the_number_of_gains(#[case] name: &str) {
    let cards = printed();
    let card = cards.iter().find(|card| card.name == name).unwrap();
    let compiled = serde_json::to_string(&mtg_cards::compile::compile(card).card.unwrap()).unwrap();
    assert!(
        compiled.contains(r#""what":"LifeGainedTotal""#),
        "{name}: {compiled}"
    );
}
