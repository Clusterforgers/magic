//! Goal: the entering abilities of round 15 pack e15b read back as the clauses their cards print (a choice of a card or a
//! basic land type, a counter of a chosen kind, tapped and with counters, spells you cast of the chosen type); another
//! list, kind, count or order of words never does.
use super::*;
use rstest::rstest;

fn e15b_reads(json: &str, clause: &str) -> bool {
    let ability: Ability = serde_json::from_str(json).expect("ability JSON");
    written_variants(&ability)
        .iter()
        .any(|wording| same_text(wording, clause))
}

const PURSE: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":{"Other":"coin"},"count":3,"tapped":true}}}}"#;
const NIPPER: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounterOf":[{"Other":"deathtouch"},{"Other":"lifelink"}]}}}"#;
const KEY: &str =
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"Choose":{"CardTypeOf":285}}}}"#;
const BASIC: &str =
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"Choose":"BasicLandType"}}}"#;
const COST: &str = r#"{"CostModifier":{"applies_to":["Spell","ChosenSubtype",{"ControlledBy":"You"}],"change":"Reduce","mana":"{1}"}}"#;

#[rstest]
#[case::tapped_and_with(PURSE, "~ enters tapped and with three coin counters on it.", true)]
#[case::tapped_with(PURSE, "~ enters tapped with three coin counters on it.", true)]
#[case::other_count(PURSE, "~ enters tapped and with two coin counters on it.", false)]
#[case::untapped(PURSE, "~ enters with three coin counters on it.", false)]
#[case::counter_choice(
    NIPPER,
    "~ enters with your choice of a deathtouch counter or a lifelink counter on it.",
    true
)]
#[case::counter_choice_other_kind(
    NIPPER,
    "~ enters with your choice of a deathtouch counter or a menace counter on it.",
    false
)]
#[case::card_types(
    KEY,
    "As ~ enters, choose artifact, creature, enchantment, instant, or sorcery.",
    true
)]
#[case::other_card_types(
    KEY,
    "As ~ enters, choose artifact, creature, enchantment, land, or sorcery.",
    false
)]
#[case::basic_land_type(BASIC, "As ~ enters, choose a basic land type.", true)]
#[case::any_land_type(BASIC, "As ~ enters, choose a land type.", false)]
#[case::cast_before_chosen(
    COST,
    "Spells you cast of the chosen type cost {1} less to cast.",
    true
)]
#[case::more_not_less(
    COST,
    "Spells you cast of the chosen type cost {1} more to cast.",
    false
)]
fn an_entering_ability_reads_back(#[case] json: &str, #[case] clause: &str, #[case] read: bool) {
    assert_eq!(e15b_reads(json, clause), read, "{clause}");
}
