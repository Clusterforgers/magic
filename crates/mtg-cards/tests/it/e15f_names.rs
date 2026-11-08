//! Goal: the rules carry the name of a mode ("• Charge Them —") and the word a Siege chooses as it enters ("choose Khans or Dragons"), and the read-back tells one mode from another: the right names read back, while a missing, wrong or swapped name, a line under the other word and a choice of other words read `Differs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use rstest::rstest;
use serde_json::{Value, json};

use mtg_cards::load::Library;
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::{Caps, PrintedCard, PrintedFace};

fn printed(name: &str, type_line: &str, text: &str) -> PrintedCard {
    PrintedCard {
        oracle_id: "7a2f6a8e-3c64-4c1e-9f0b-5e15f0000001".into(),
        name: name.into(),
        faces: vec![PrintedFace {
            name: name.into(),
            type_line: type_line.into(),
            oracle_text: text.into(),
            ..PrintedFace::default()
        }],
        ..PrintedCard::default()
    }
}

fn verdict(card: &PrintedCard, abilities: &Value) -> Verdict {
    let rules = json!({"faces": [{"abilities": abilities}]}).to_string();
    check_exact(
        &rules,
        card,
        &Library::default(),
        &BTreeSet::new(),
        &Caps::everything(),
    )
    .verdict()
}

const GOBLINS: &str = "Choose one \u{2014}\n\u{2022} Charge Them \u{2014} Draw a card.\n\u{2022} Befriend Them \u{2014} You gain 2 life.";

fn charm(labels: [Option<&str>; 2]) -> Value {
    let effects = [json!({"Draw": {}}), json!({"GainLife": {"amount": 2}})];
    let modes: Vec<Value> = labels
        .iter()
        .zip(effects)
        .map(|(label, effect)| match label {
            Some(label) => json!({"label": label, "effects": [effect]}),
            None => json!({"effects": [effect]}),
        })
        .collect();
    json!([{"Spell": {"effects": [{"Modal": {"modes": modes}}]}}])
}

#[rstest]
#[case::named(GOBLINS, [Some("Charge Them"), Some("Befriend Them")], Verdict::Accepted)]
#[case::a_card_without_names(
    "Choose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.",
    [Some("Charge Them"), Some("Befriend Them")],
    Verdict::Accepted
)]
#[case::names_missing(GOBLINS, [None, None], Verdict::Mismatch)]
#[case::one_name_missing(GOBLINS, [Some("Charge Them"), None], Verdict::Mismatch)]
#[case::a_wrong_name(GOBLINS, [Some("Charge It"), Some("Befriend Them")], Verdict::Mismatch)]
#[case::another_case(GOBLINS, [Some("Charge them"), Some("Befriend Them")], Verdict::Mismatch)]
#[case::names_swapped(GOBLINS, [Some("Befriend Them"), Some("Charge Them")], Verdict::Mismatch)]
fn e15f_a_mode_reads_back_with_its_name(
    #[case] text: &str,
    #[case] labels: [Option<&str>; 2],
    #[case] expected: Verdict,
) {
    let card = printed("Goblin Choice", "Instant", text);
    assert_eq!(verdict(&card, &charm(labels)), expected);
}

const SIEGE: &str = "As Test Siege enters, choose Khans or Dragons.\n\u{2022} Khans \u{2014} At the beginning of your upkeep, draw a card.\n\u{2022} Dragons \u{2014} At the beginning of your upkeep, you gain 2 life.";

const SIEGE_THIS: &str = "As this enchantment enters, choose Khans or Dragons.\n\u{2022} Khans \u{2014} At the beginning of your upkeep, draw a card.\n\u{2022} Dragons \u{2014} At the beginning of your upkeep, you gain 2 life.";

fn upkeep(effect: &Value, condition: Option<Value>) -> Value {
    let mut trigger =
        json!({"when": {"StepBegins": {"step": "Upkeep", "whose": "You"}}, "effects": [effect]});
    if let Some(condition) = condition {
        trigger["if"] = condition;
    }
    json!({ "Triggered": trigger })
}

fn siege(words: [&str; 2], under: [Option<&str>; 2]) -> Value {
    let entered = |word: Option<&str>| word.map(|word| json!({ "EnteredAs": word }));
    json!([
        {"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": {"ChooseWord": words}}},
        upkeep(&json!({"Draw": {}}), entered(under[0])),
        upkeep(&json!({"GainLife": {"amount": 2}}), entered(under[1])),
    ])
}

#[rstest]
#[case::by_name(SIEGE, ["Khans", "Dragons"], [Some("Khans"), Some("Dragons")], Verdict::Accepted)]
#[case::this_enchantment(SIEGE_THIS, ["Khans", "Dragons"], [Some("Khans"), Some("Dragons")], Verdict::Accepted)]
#[case::lines_swapped(SIEGE, ["Khans", "Dragons"], [Some("Dragons"), Some("Khans")], Verdict::Mismatch)]
#[case::both_under_one_word(SIEGE, ["Khans", "Dragons"], [Some("Khans"), Some("Khans")], Verdict::Mismatch)]
#[case::a_line_without_its_word(SIEGE, ["Khans", "Dragons"], [Some("Khans"), None], Verdict::Mismatch)]
#[case::another_word(SIEGE, ["Khans", "Dragons"], [Some("Khans"), Some("Dragon")], Verdict::Mismatch)]
#[case::another_choice(SIEGE, ["Khans", "Dragon"], [Some("Khans"), Some("Dragons")], Verdict::Mismatch)]
#[case::words_in_another_order(SIEGE, ["Dragons", "Khans"], [Some("Khans"), Some("Dragons")], Verdict::Mismatch)]
fn e15f_a_siege_reads_back_with_its_words(
    #[case] text: &str,
    #[case] words: [&str; 2],
    #[case] under: [Option<&str>; 2],
    #[case] expected: Verdict,
) {
    let card = printed("Test Siege", "Enchantment", text);
    assert_eq!(verdict(&card, &siege(words, under)), expected);
}

#[test]
fn e15f_the_compiler_reads_a_siege_as_the_choice_and_a_line_for_each_word() {
    let card = printed("Test Siege", "Enchantment", SIEGE);
    let compiled = mtg_cards::compile::compile(&card)
        .card
        .expect("the siege reads");
    let abilities = serde_json::to_value(&compiled.faces[0].abilities).unwrap();
    let words: Vec<&Value> = abilities
        .as_array()
        .unwrap()
        .iter()
        .map(|ability| {
            ability
                .pointer("/Triggered/if/EnteredAs")
                .or_else(|| ability.pointer("/Replacement/by/ChooseWord"))
                .unwrap()
        })
        .collect();
    assert_eq!(
        words,
        [
            &json!(["Khans", "Dragons"]),
            &json!("Khans"),
            &json!("Dragons")
        ]
    );
}
