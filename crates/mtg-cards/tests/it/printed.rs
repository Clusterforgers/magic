//! Goal: a rules file is checked against the imported card (names, fingerprints, keywords, printed types and cost), and each kind of mismatch is reported at the path of the value that causes it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::load::{Library, Source};
use mtg_cards::support::{PrintedCard, PrintedFace, oracle_fingerprint};
use rstest::rstest;

const ID: &str = "4457ed35-7c10-48c8-9776-456485fdf070";

fn printed(type_line: &str, mana_cost: &str, text: &str, keywords: &[&str]) -> PrintedCard {
    PrintedCard {
        oracle_id: ID.into(),
        name: "Test Card".into(),
        keywords: keywords.iter().map(|k| (*k).to_owned()).collect(),
        faces: vec![PrintedFace {
            name: "Test Card".into(),
            mana_cost: mana_cost.into(),
            type_line: type_line.into(),
            oracle_text: text.into(),
            ..PrintedFace::default()
        }],
    }
}

fn library(status: &str, abilities: &str, fingerprint: &str) -> Library {
    let json = format!(
        r#"{{"oracle_id":"{ID}","name":"Test Card","impl_status":"{status}","faces":[{{"name":"Test Card","oracle_fingerprint":"{fingerprint}","abilities":[{abilities}]}}]}}"#
    );
    Library::from_sources(&[Source {
        path: "cards/test_card.json".into(),
        text: json,
    }])
    .unwrap_or_else(|errors| panic!("{errors}"))
}

fn problems(status: &str, abilities: &str, shown: &PrintedCard) -> Vec<String> {
    let fingerprint = oracle_fingerprint(&shown.faces[0].oracle_text);
    library(status, abilities, &fingerprint)
        .check_printed(std::slice::from_ref(shown))
        .iter()
        .map(|e| format!("{}: {}", e.path, e.message))
        .collect()
}

const DAMAGE: &str = r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}"#;
const X_DAMAGE: &str = r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"X"}}]}}"#;

#[rstest]
#[case::clean(printed("Instant", "{R}", "Deal 3.", &[]), DAMAGE, &[])]
#[case::x_with_an_x_cost(printed("Sorcery", "{X}{R}", "Deal X.", &[]), X_DAMAGE, &[])]
#[case::x_without_an_x_cost(
    printed("Sorcery", "{R}", "Deal X.", &[]),
    X_DAMAGE,
    &["faces[0].abilities[0].Spell.effects[0].DealDamage.amount: uses X but the imported mana cost has none"]
)]
#[case::spell_on_a_creature(
    printed("Creature \u{2014} Elf", "{G}", "Deal 3.", &[]),
    DAMAGE,
    &["faces[0].abilities: a Spell ability belongs to an instant or sorcery"]
)]
#[case::instant_without_a_spell_ability(
    printed("Instant", "{R}", "Deal 3.", &[]),
    r#"{"Keyword":"Flash"}"#,
    &["faces[0].abilities: an instant or sorcery needs a Spell ability"]
)]
#[case::loyalty_off_a_planeswalker(
    printed("Creature \u{2014} Elf", "{G}", "+1: Draw.", &[]),
    r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"Draw":{}}]}}"#,
    &["faces[0].abilities[0]: a Loyalty ability belongs to a planeswalker"]
)]
#[case::loyalty_on_a_planeswalker(
    printed("Legendary Planeswalker \u{2014} Jace", "{2}{U}", "+1: Draw.", &[]),
    r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"Draw":{}}]}}"#,
    &[]
)]
#[case::enchant_without_aura(
    printed("Enchantment", "{1}{W}", "Enchant creature", &["Enchant"]),
    r#"{"Keyword":{"Enchant":[{"Type":"Creature"}]}}"#,
    &["faces[0].abilities[0]: Enchant belongs to an Aura"]
)]
#[case::keyword_missing(
    printed("Creature \u{2014} Bird", "{U}", "Flying", &["Flying"]),
    r#"{"Keyword":"Reach"}"#,
    &["the imported card has the keyword `Flying` but no face has it"]
)]
#[case::keyword_found_in_any_case(
    printed("Creature \u{2014} Bird", "{U}", "Flying", &["flying"]),
    r#"{"Keyword":"Flying"}"#,
    &[]
)]
#[case::action_that_is_not_a_keyword_ability(
    printed("Instant", "{U}", "Scry 1.", &["Scry"]),
    r#"{"Spell":{"effects":[{"Scry":{}}]}}"#,
    &[]
)]
#[case::clause_beyond_the_text(
    printed("Creature \u{2014} Bird", "{U}", "Flying", &[]),
    r#"{"Keyword":"Flying"},{"Manual":{"reason":"x","span":{"start":2,"end":99}}}"#,
    &["the span ends past the oracle text (6 characters)"]
)]
#[case::unknown_card_name(
    printed("Instant", "{U}", "Search.", &[]),
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"Named":"Nonexistent Card"}],"as":"c"}}]}}"#,
    &["no imported card is named `Nonexistent Card`"]
)]
#[case::known_card_name(
    printed("Instant", "{U}", "Search.", &[]),
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"Named":"test card"}],"as":"c"}}]}}"#,
    &[]
)]
fn each_mismatch_is_reported_where_it_is(
    #[case] shown: PrintedCard,
    #[case] abilities: &str,
    #[case] expected: &[&str],
) {
    let status = if abilities.contains("Manual") {
        "Partial"
    } else {
        "Full"
    };
    let found = problems(status, abilities, &shown);
    if expected.is_empty() {
        assert!(found.is_empty(), "{found:#?}");
    }
    for want in expected {
        assert!(
            found.iter().any(|f| f.contains(want)),
            "wanted {want:?} in {found:#?}"
        );
    }
}

#[test]
fn drift_names_both_fingerprints() {
    let shown = printed("Instant", "{R}", "Deal 4.", &[]);
    let found = library("Full", DAMAGE, &oracle_fingerprint("Deal 3."))
        .check_printed(&[shown])
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].starts_with("cards/test_card.json: faces[0].oracle_fingerprint: the oracle text changed since the rules were written"),
        "{found:#?}"
    );
}

#[rstest]
#[case::name("Other Card", 1, "name: the imported name is `Other Card`")]
#[case::faces("Test Card", 2, "faces: the imported card has 2 face(s), the file 1")]
fn names_and_face_counts_must_agree(
    #[case] name: &str,
    #[case] faces: usize,
    #[case] expected: &str,
) {
    let mut shown = printed("Instant", "{R}", "Deal 3.", &[]);
    shown.name = name.into();
    shown.faces = vec![shown.faces[0].clone(); faces];
    let found = library("Full", DAMAGE, &oracle_fingerprint("Deal 3."))
        .check_printed(&[shown])
        .iter()
        .map(|e| format!("{}: {}", e.path, e.message))
        .collect::<Vec<_>>();
    assert!(found.iter().any(|f| f.contains(expected)), "{found:#?}");
}

#[test]
fn a_file_with_no_imported_card_is_reported() {
    let found = library("Manual", "", "").check_printed(&[]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, "oracle_id");
}

#[test]
fn manual_cards_are_not_held_to_a_fingerprint_or_keywords() {
    let shown = printed(
        "Creature \u{2014} Bird",
        "{U}",
        "Flying, and more.",
        &["Flying"],
    );
    assert_eq!(
        library("Manual", "", "").check_printed(&[shown]),
        Vec::new()
    );
}
