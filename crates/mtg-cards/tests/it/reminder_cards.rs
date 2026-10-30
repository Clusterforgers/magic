//! Goal: a card whose whole text is reminder text of what its types give (the original dual lands, the basics, Dryad Arbor) compiles to nothing, is `Verified` with no rules and plays the mana of its basic land types, a card with rules text beside the reminder is not that, and a proposal for such a card is accepted exactly when it adds nothing the types do not give. (t19b)
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::assemble::assemble;
use mtg_cards::compile::compile;
use mtg_cards::definition::AbilityDef;
use mtg_cards::generated::{Generation, generate};
use mtg_cards::load::Library;
use mtg_cards::proposal::{RepairClass, Report, Verdict, check};
use mtg_cards::support::{PrintedCard, PrintedFace, Support, has_rules_text, support};
use mtg_core::mana::ManaType::{self, Black, Blue, Green, Red, White};
use rstest::rstest;
use serde_json::json;

fn land(name: &str, type_line: &str, text: &str) -> PrintedCard {
    PrintedCard {
        oracle_id: "00000000-0000-0000-0000-000000000019".to_owned(),
        name: name.to_owned(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            type_line: type_line.to_owned(),
            oracle_text: text.to_owned(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[rstest]
#[case::volcanic_island("Volcanic Island", "Land \u{2014} Island Mountain", "({T}: Add {U} or {R}.)", &[Blue, Red])]
#[case::tundra("Tundra", "Land \u{2014} Plains Island", "({T}: Add {W} or {U}.)", &[White, Blue])]
#[case::tropical_island("Tropical Island", "Land \u{2014} Forest Island", "({T}: Add {G} or {U}.)", &[Blue, Green])]
#[case::badlands("Badlands", "Land \u{2014} Swamp Mountain", "({T}: Add {B} or {R}.)", &[Black, Red])]
#[case::savannah("Savannah", "Land \u{2014} Forest Plains", "({T}: Add {G} or {W}.)", &[White, Green])]
#[case::taiga("Taiga", "Land \u{2014} Mountain Forest", "({T}: Add {R} or {G}.)", &[Red, Green])]
#[case::bayou("Bayou", "Land \u{2014} Swamp Forest", "({T}: Add {B} or {G}.)", &[Black, Green])]
#[case::scrubland("Scrubland", "Land \u{2014} Plains Swamp", "({T}: Add {W} or {B}.)", &[White, Black])]
#[case::plateau("Plateau", "Land \u{2014} Mountain Plains", "({T}: Add {R} or {W}.)", &[White, Red])]
#[case::underground_sea("Underground Sea", "Land \u{2014} Island Swamp", "({T}: Add {U} or {B}.)", &[Blue, Black])]
#[case::a_basic("Forest", "Basic Land \u{2014} Forest", "({T}: Add {G}.)", &[Green])]
#[case::a_snow_basic("Snow-Covered Island", "Basic Snow Land \u{2014} Island", "({T}: Add {U}.)", &[Blue])]
#[case::dryad_arbor(
    "Dryad Arbor",
    "Land Creature \u{2014} Forest Dryad",
    "(This land isn't a spell, it's affected by summoning sickness, and it has \"{T}: Add {G}.\")",
    &[Green]
)]
fn a_card_of_reminder_text_only_is_verified_with_no_rules_and_taps_for_its_types(
    #[case] name: &str,
    #[case] type_line: &str,
    #[case] text: &str,
    #[case] mana: &[ManaType],
) {
    let printed = land(name, type_line, text);
    let caps = Caps::everything();
    assert!(!has_rules_text(text));
    assert!(compile(&printed).card.is_none());
    assert_eq!(
        generate(&printed, &Library::default(), &BTreeSet::new()),
        Generation::Nothing
    );
    assert_eq!(support(None, &printed, &caps).support, Support::Verified);
    let assembled = assemble(&printed, None, &caps);
    assert_eq!(assembled.support.support, Support::Verified);
    assert!(assembled.definition.manual.is_empty());
    let given: Vec<ManaType> = assembled
        .definition
        .abilities
        .iter()
        .map(|AbilityDef::Mana(ability)| ability.produces)
        .collect();
    assert_eq!(
        given.len(),
        mana.len(),
        "one mana ability per basic land type"
    );
    assert!(given.iter().all(|one| mana.contains(one)), "{given:?}");
}

#[rstest]
#[case::a_shock_land_with_its_replacement(
    "Land \u{2014} Island Mountain",
    "({T}: Add {U} or {R}.)\nAs this land enters, you may pay 2 life. If you don't, it enters tapped."
)]
#[case::a_triome_with_cycling(
    "Land \u{2014} Mountain Forest Plains",
    "({T}: Add {R}, {G}, or {W}.)\nThis land enters tapped.\nCycling {3} ({3}, Discard this card: Draw a card.)"
)]
#[case::wastes_prints_its_mana("Basic Land", "{T}: Add {C}.")]
#[case::a_land_with_an_extra_ability(
    "Land \u{2014} Forest",
    "({T}: Add {G}.)\n{1}, {T}, Sacrifice this land: Draw a card."
)]
#[case::a_reminder_that_never_closes("Land \u{2014} Forest", "({T}: Add {G}.")]
fn a_card_with_rules_text_beside_the_reminder_is_not_a_card_of_reminder_text_only(
    #[case] type_line: &str,
    #[case] text: &str,
) {
    let printed = land("Card", type_line, text);
    assert!(has_rules_text(text));
    assert_ne!(
        support(None, &printed, &Caps::everything()).support,
        Support::Verified
    );
}

fn judge(printed: &PrintedCard, rules: &str) -> Report {
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let names: BTreeSet<String> = std::iter::once(printed.name.to_lowercase()).collect();
    check(rules, printed, &tokens, &names, &Caps::everything())
}

fn volcanic_island() -> PrintedCard {
    land(
        "Volcanic Island",
        "Land \u{2014} Island Mountain",
        "({T}: Add {U} or {R}.)",
    )
}

fn tap_for(mana: &serde_json::Value) -> String {
    json!({ "faces": [{ "abilities": [
        { "Activated": { "cost": ["Tap"], "effects": [{ "AddMana": { "mana": mana } }] } }
    ] }] })
    .to_string()
}

#[rstest]
#[case::no_ability(r#"{"faces":[{"abilities":[]}]}"#.to_owned(), false)]
#[case::one_of(tap_for(&json!({ "OneOf": ["Blue", "Red"] })), true)]
#[case::one_of_lists(tap_for(&json!({ "OneOfLists": [["Blue"], ["Red"]] })), true)]
#[case::the_two_mana_apart(tap_for(&json!({ "Fixed": ["Blue"] })), true)]
fn a_proposal_that_adds_nothing_the_types_do_not_give_is_accepted(
    #[case] rules: String,
    #[case] repaired: bool,
) {
    let report = judge(&volcanic_island(), &rules);
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{}",
        report.render(true)
    );
    assert_eq!(
        report.support.as_ref().map(|s| s.support),
        Some(Support::Verified)
    );
    assert!(report.rules.as_ref().unwrap().faces[0].abilities.is_empty());
    assert_eq!(
        report
            .repairs
            .iter()
            .any(|repair| repair.class == RepairClass::ImpliedMana),
        repaired
    );
}

#[rstest]
#[case::both_mana_at_once(tap_for(&json!({ "Fixed": ["Green", "Blue"] })))]
#[case::a_color_the_land_does_not_give(tap_for(&json!({ "OneOf": ["Blue", "Black"] })))]
#[case::a_keyword_the_text_does_not_print(json!({ "faces": [{ "abilities": [{ "Keyword": "Trample" }] }] }).to_string())]
fn a_proposal_that_adds_something_is_still_a_mismatch(#[case] rules: String) {
    let report = judge(&volcanic_island(), &rules);
    assert_eq!(
        report.verdict(),
        Verdict::Mismatch,
        "{}",
        report.render(true)
    );
    assert!(!report.strays.is_empty());
}
