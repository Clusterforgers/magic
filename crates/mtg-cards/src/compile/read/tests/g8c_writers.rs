//! Goal: the mana spent to cast a spell writes as the card prints it: "{W}{W} was spent to cast it", "{C} wasn't spent",
//! "at least four mana" or "no mana was spent to cast it" after a cast; another amount, type or comparison does not read back.
use super::*;

/// The wordings the checker accepts for the rules, for the message of a failed golden.
fn wordings(json: &str) -> Vec<String> {
    written_variants(&ability(json))
}

fn enters(condition: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"to":"Battlefield","what":["This"]}}}},"if":{condition},"effects":[{{"Draw":{{}}}}]}}}}"#
    )
}

fn cast(condition: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"SpellCast":{{"by":"You","what":[{{"Not":{{"Type":"Creature"}}}}]}}}},"if":{condition},"effects":[{{"Draw":{{}}}}]}}}}"#
    )
}

const TWO_WHITE: &str = r#"{"Compare":{"left":{"ManaSpent":{"of":"White"}},"op":"Ge","right":2}}"#;
const THREE_WHITE: &str =
    r#"{"Compare":{"left":{"ManaSpent":{"of":"White"}},"op":"Ge","right":3}}"#;
const TWO_RED: &str = r#"{"Compare":{"left":{"ManaSpent":{"of":"Red"}},"op":"Ge","right":2}}"#;
const NO_COLORLESS: &str = r#"{"Not":{"ManaSpent":"Colorless"}}"#;
const COLORLESS: &str = r#"{"ManaSpent":"Colorless"}"#;
const FOUR: &str =
    r#"{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"ManaSpent"}},"op":"Ge","right":4}}"#;
const FIVE: &str =
    r#"{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"ManaSpent"}},"op":"Ge","right":5}}"#;
const NONE: &str =
    r#"{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"ManaSpent"}},"op":"Eq","right":0}}"#;
const NONE_OF_THIS: &str =
    r#"{"Compare":{"left":{"Stat":{"of":"This","stat":"ManaSpent"}},"op":"Eq","right":0}}"#;

#[rstest]
#[case::two_white(
    enters(TWO_WHITE),
    "When ~ enters, if {W}{W} was spent to cast it, draw a card.",
    true
)]
#[case::three_is_not_two(
    enters(THREE_WHITE),
    "When ~ enters, if {W}{W} was spent to cast it, draw a card.",
    false
)]
#[case::red_is_not_white(
    enters(TWO_RED),
    "When ~ enters, if {W}{W} was spent to cast it, draw a card.",
    false
)]
#[case::mixed_symbols(
    enters(TWO_WHITE),
    "When ~ enters, if {W}{U} was spent to cast it, draw a card.",
    false
)]
#[case::not_spent(
    enters(NO_COLORLESS),
    "When ~ enters, if {C} wasn't spent to cast it, draw a card.",
    true
)]
#[case::spent_is_not_unspent(
    enters(COLORLESS),
    "When ~ enters, if {C} wasn't spent to cast it, draw a card.",
    false
)]
#[case::at_least_four(
    cast(FOUR),
    "Whenever you cast a noncreature spell, if at least four mana was spent to cast it, draw a card.",
    true
)]
#[case::five_is_not_four(
    cast(FIVE),
    "Whenever you cast a noncreature spell, if at least four mana was spent to cast it, draw a card.",
    false
)]
#[case::no_mana(
    cast(NONE),
    "Whenever you cast a noncreature spell, if no mana was spent to cast it, draw a card.",
    true
)]
#[case::no_mana_that_spell(
    cast(NONE),
    "Whenever you cast a noncreature spell, if no mana was spent to cast that spell, draw a card.",
    true
)]
#[case::the_spell_not_this(
    cast(NONE_OF_THIS),
    "Whenever you cast a noncreature spell, if no mana was spent to cast it, draw a card.",
    false
)]
#[case::four_is_not_none(
    cast(FOUR),
    "Whenever you cast a noncreature spell, if no mana was spent to cast it, draw a card.",
    false
)]
fn g8c_mana_spent_writes_as_printed(#[case] json: String, #[case] text: &str, #[case] reads: bool) {
    let found = wordings(&json);
    let written = found.iter().any(|wording| same_text(wording, text));
    assert_eq!(written, reads, "{found:?}");
    // The reader gives the rule back from the printed clause, and no other rule.
    let read = clause(text, ClauseKind::Triggered, false);
    assert_eq!(read.as_ref() == Some(&ability(&json)), reads, "{read:?}");
}
