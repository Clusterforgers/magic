//! Goal: the object verbs read the shapes that finished no card before: populate, token lists and colours, counter words, attach choices, and clean-up only pumps.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a sorcery with the given lines as compact JSON, and its gaps.
fn spell(lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Sorcery\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
#[case::populate(
    "A:SP$ CopyPermanent | Populate$ True",
    &[r#"{"Choose":{"as":"populated","from":{"Each":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]}}"#, r#"{"Bound":"populated"}"#]
)]
#[case::populate_attacking(
    "A:SP$ CopyPermanent | Populate$ True | TokenTapped$ True | TokenAttacking$ True",
    &[r#""attacking":true"#, r#""tapped":true"#]
)]
#[case::token_list(
    "A:SP$ Token | TokenScript$ w_1_1_soldier,g_2_2_boar_trample | TokenOwner$ You",
    &[r#"{"CreateToken":{"#, r#""name":"Soldier""#, r#""name":"Boar""#]
)]
#[case::colorless_token(
    "A:SP$ Token | TokenScript$ c_1_1_a_thopter_flying",
    &[r#""name":"Thopter""#]
)]
#[case::defined_stats(
    "A:SP$ Token | TokenScript$ g_x_x_treefolk_forests",
    &[r#""power":0"#, r#""defining":true"#]
)]
#[case::remove_any(
    "A:SP$ RemoveCounter | ValidTgts$ Permanent | CounterType$ Any | CounterNum$ 2",
    &[r#"{"RemoveCounters":{"count":2,"from":{"Target":0}}}"#]
)]
#[case::remove_all(
    "A:SP$ RemoveCounter | ValidTgts$ Creature | CounterType$ All | CounterNum$ All",
    &[r#""RemoveAllCounters""#]
)]
#[case::multiply_each_kind(
    "A:SP$ MultiplyCounter | ValidTgts$ Permanent | Multiplier$ 2",
    &[r#"{"MultiplyCounters":{"on":{"Target":0}}}"#]
)]
#[case::pump_cleanup_only(
    "A:SP$ Pump | ValidTgts$ Creature | AtEOT$ Sacrifice",
    &[r#""DelayedTrigger""#, r#""Sacrifice""#]
)]
#[case::counters_all_at_once(
    "A:SP$ PutCounter | ValidTgts$ Creature | CounterTypes$ Flying,Lifelink",
    &[r#""counter":{"Other":"flying"}"#, r#""counter":{"Other":"lifelink"}"#]
)]
#[case::attach_the_chosen_attachment(
    "A:SP$ Attach | Choices$ Equipment.YouCtrl | Defined$ Self | Optional$ True",
    &[r#""May""#, r#""Choose""#, r#""Attach":{"to":"This","what":{"Bound":"#]
)]
#[case::attach_to_a_chosen_host(
    "A:SP$ Attach | Choices$ Creature.YouCtrl | Object$ Self",
    &[r#""Choose""#, r#""Attach":{"to":{"Bound":"#, r#""what":"This""#]
)]
#[case::damage_all_from_a_source(
    "A:SP$ DamageAll | ValidCards$ Creature | NumDmg$ 2 | DamageSource$ Self",
    &[r#""source":"This""#, r#""DealDamage""#]
)]
#[case::move_from_the_target(
    "A:SP$ MoveCounter | ValidTgts$ Creature | Defined$ Self | CounterType$ P1P1 | CounterNum$ 1",
    &[r#"{"MoveCounters":{"counter":"+1/+1","from":{"Target":0},"to":"This""#]
)]
fn the_shape_is_read(#[case] lines: &str, #[case] parts: &[&str]) {
    let (json, found) = spell(lines);
    assert!(found.is_empty(), "{found:?}");
    for part in parts {
        assert!(json.contains(part), "{part} missing from {json}");
    }
}

#[test]
fn a_colorless_token_script_gives_no_color() {
    let (json, found) = spell("A:SP$ Token | TokenScript$ c_1_1_a_thopter_flying");
    assert!(found.is_empty(), "{found:?}");
    assert!(!json.contains("colors"), "{json}");
}

#[rstest]
#[case::spirit_stats_have_no_defining_line(
    "A:SP$ Token | TokenScript$ w_x_x_spirit",
    "token:variable-stats"
)]
#[case::any_with_all(
    "A:SP$ RemoveCounter | ValidTgts$ Permanent | CounterType$ Any | CounterNum$ All",
    "key:RemoveCounter.Any-number"
)]
#[case::attach_needs_one_end(
    "A:SP$ Attach | Choices$ Creature | Object$ Self | Defined$ Self",
    "key:Attach.Choices-shape"
)]
#[case::token_list_with_sizes(
    "A:SP$ Token | TokenScript$ w_1_1_soldier,g_2_2_boar_trample | TokenPower$ 3 | TokenToughness$ 3",
    "key:Token.TokenScript-list"
)]
fn an_unreadable_shape_stays_a_named_gap(#[case] lines: &str, #[case] gap: &str) {
    let (_, found) = spell(lines);
    assert_eq!(found.first().map(String::as_str), Some(gap), "{found:?}");
}
