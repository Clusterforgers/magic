//! Goal: mana whose rider gives the creature spell it pays for a +1/+1 counter or haste is written as the cards word it
//! ("If that mana is spent on a creature spell, it gains haste until end of turn"), and another kind of spell, a missing
//! or added duration, or counters in place of haste is not its text.
use super::*;

/// Whether one of the wordings of the ability is the text.
fn writes(json: &str, text: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

/// A tap ability that adds `mana` with the rider `bonus`.
fn rider(mana: &str, bonus: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{mana},"only":[{{"Enters":{bonus}}}]}}}}]}}}}"#
    )
}

const ANY: &str = r#"{"AnyColor":1}"#;
const TWO: &str = r#"{"Fixed":["Colorless","Colorless"]}"#;
const ONE: &str = r#"{"Fixed":["Colorless"]}"#;
const COUNTER: &str = r#"{"spell":{"types":["Creature"]},"counters":1}"#;
const MULTICOLORED: &str = r#"{"spell":{"types":["Creature"],"multicolored":true},"counters":1}"#;
const HASTE: &str = r#"{"spell":{"types":["Creature"]},"keyword":"Haste","this_turn":true}"#;
const HASTE_FOR_GOOD: &str = r#"{"spell":{"types":["Creature"]},"keyword":"Haste"}"#;

#[rstest]
#[case::biophagus(
    ANY,
    COUNTER,
    "{T}: Add one mana of any color. If this mana is spent to cast a creature spell, that creature enters with an additional +1/+1 counter on it."
)]
#[case::guildmages_forum(
    ANY,
    MULTICOLORED,
    "{T}: Add one mana of any color. If that mana is spent on a multicolored creature spell, that creature enters with an additional +1/+1 counter on it."
)]
#[case::generator_servant(
    TWO,
    HASTE,
    "{T}: Add {C}{C}. If any of that mana is spent on a creature spell, it gains haste until end of turn."
)]
#[case::hall_of_the_bandit_lord(
    ONE,
    HASTE_FOR_GOOD,
    "{T}: Add {C}. If that mana is spent on a creature spell, it gains haste."
)]
fn a_rider_is_written_as_the_card_words_it(
    #[case] mana: &str,
    #[case] bonus: &str,
    #[case] text: &str,
) {
    let json = rider(mana, bonus);
    assert!(
        writes(&json, text),
        "{text}: {:?}",
        render_variants(&ability(&json))
    );
}

#[rstest]
#[case::not_multicolored(
    ANY,
    COUNTER,
    "{T}: Add one mana of any color. If that mana is spent on a multicolored creature spell, that creature enters with an additional +1/+1 counter on it."
)]
#[case::haste_without_its_duration(
    ONE,
    HASTE_FOR_GOOD,
    "{T}: Add {C}. If that mana is spent on a creature spell, it gains haste until end of turn."
)]
#[case::haste_with_a_duration(
    ONE,
    HASTE,
    "{T}: Add {C}. If that mana is spent on a creature spell, it gains haste."
)]
#[case::a_counter_is_not_haste(
    ANY,
    COUNTER,
    "{T}: Add one mana of any color. If that mana is spent on a creature spell, it gains haste."
)]
#[case::an_artifact_spell(
    ONE,
    HASTE,
    "{T}: Add {C}. If that mana is spent on an artifact spell, it gains haste until end of turn."
)]
fn another_rider_is_not_the_text(#[case] mana: &str, #[case] bonus: &str, #[case] text: &str) {
    assert!(!writes(&rider(mana, bonus), text));
}
