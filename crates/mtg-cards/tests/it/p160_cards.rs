//! Goal: every card the p160 pack made fully automatic reads back as its printed text clause by clause through
//! `check_exact`, and a near miss (another activator, amount, zone quality, step, restriction or color) does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn p160_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn p160_verdict(name: &str, text: &str) -> Verdict {
    let cards = common::printed();
    let names: BTreeSet<String> = cards
        .iter()
        .flat_map(|card: &PrintedCard| {
            std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name))
        })
        .map(|name| name.to_lowercase())
        .collect();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards.iter().find(|card| card.name == name).unwrap();
    check_exact(text, card, &tokens, &names, &Caps::everything()).verdict()
}

#[rstest]
#[case("Master Warcraft")]
#[case("Martyrdom")]
#[case("Close Encounter")]
#[case("Ghostly Flame")]
#[case("Guardian Beast")]
fn p160_reads_back_whole(#[case] name: &str) {
    assert_eq!(p160_verdict(name, &p160_file(name)), Verdict::Accepted, "{name}");
}

#[rstest]
#[case::martyrdom_anyone("Martyrdom", r#""R6022Granter""#, r#""Anyone""#)]
#[case::martyrdom_two_damage("Martyrdom", r#""most": 1"#, r#""most": 2"#)]
#[case::encounter_any_exiled_card("Close Encounter", r#""R702185Warped", "#, "")]
#[case::warcraft_after_blockers("Master Warcraft", r#""BeforeAttackers""#, r#""AfterBlockers""#)]
#[case::warcraft_no_attackers(
    "Master Warcraft",
    r#""R5081ChoosesAttackers": { "chooser": "You" }"#,
    r#""R6400ChoosesVotes": { "chooser": "You" }"#
)]
#[case::flame_green("Ghostly Flame", r#"{ "Color": "Red" }"#, r#"{ "Color": "Green" }"#)]
#[case::beast_while_tapped("Guardian Beast", r#"["Untapped"]"#, r#"["Tapped"]"#)]
#[case::beast_control_free("Guardian Beast", r#", "R1012ControlKept""#, "")]
#[case::beast_enchantable("Guardian Beast", r#""R3034CantBeEnchanted", "#, "")]
fn p160_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = p160_file(name);
    assert!(file.contains(from), "{name}: the file has {from}");
    assert_ne!(
        p160_verdict(name, &file.replacen(from, to, 1)),
        Verdict::Accepted,
        "{name}: {from} -> {to}"
    );
}
