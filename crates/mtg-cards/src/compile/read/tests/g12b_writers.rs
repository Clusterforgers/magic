//! Goal: "put its counters on target creature you control" and "for each kind of counter on target permanent or player, give that permanent or player another counter of that kind" write as the card prints them and the first reads back to the same rule; another object read, another place, a number of each kind or a target that is not the one read does not read back.
use super::*;

const TO_YOURS: &str =
    r#""targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}]"#;

fn dies(of: &str, each: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"from":"Battlefield","to":"Graveyard","what":["This"]}}}},"effects":[{{"PutCountersOf":{{"on":{{"Target":0}},"of":{of}{each}}}}}],{TO_YOURS}}}}}"#
    )
}

fn another_dies(of: &str, on: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"from":"Battlefield","to":"Graveyard","what":[{{"Type":"Creature"}},{{"ControlledBy":"You"}},"Other"]}}}},"effects":[{{"PutCountersOf":{{"on":{on},"of":{of}}}}}]}}}}"#
    )
}

fn each_kind(on: &str, of: &str, each: u32) -> String {
    format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"PutCountersOf":{{"on":{on},"of":{of},"each":{each}}}}}],"targets":[{{"what":{{"Either":[{{"Object":[]}},"Player"]}}}}]}}}}"#
    )
}

const PUPIL: &str = "When ~ dies, put its counters on target creature you control.";
const COLONY: &str = "Whenever another creature you control dies, put its counters on ~.";
const COLONY_THOSE: &str = "Whenever another creature you control dies, put those counters on ~.";
const BROKER: &str = "{T}: For each kind of counter on target permanent or player, give that permanent or player another counter of that kind.";

#[rstest]
#[case::the_card_itself(dies(r#""This""#, ""), PUPIL, true)]
#[case::the_object_of_the_event(dies(r#""Triggering""#, ""), PUPIL, true)]
#[case::one_of_each_kind_is_not_its_counters(dies(r#""This""#, r#","each":1"#), PUPIL, false)]
#[case::the_target_s_counters_are_not_its(dies(r#"{"Target":0}"#, ""), PUPIL, false)]
#[case::another_creature_s(another_dies(r#""Triggering""#, r#""This""#), COLONY, true)]
#[case::those_counters(another_dies(r#""Triggering""#, r#""This""#), COLONY_THOSE, true)]
#[case::the_card_s_own_are_not_the_other_s(another_dies(r#""This""#, r#""This""#), COLONY, false)]
#[case::onto_the_other_is_not_onto_the_card(
    another_dies(r#""Triggering""#, r#""Triggering""#),
    COLONY,
    false
)]
#[case::another_of_each_kind(each_kind(r#"{"Target":0}"#, r#"{"Target":0}"#, 1), BROKER, true)]
#[case::two_of_each_kind(each_kind(r#"{"Target":0}"#, r#"{"Target":0}"#, 2), BROKER, false)]
#[case::the_card_s_kinds(each_kind(r#"{"Target":0}"#, r#""This""#, 1), BROKER, false)]
#[case::onto_the_card(each_kind(r#""This""#, r#"{"Target":0}"#, 1), BROKER, false)]
fn g12b_counters_of_write_as_printed(
    #[case] json: String,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability(&json));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

#[rstest]
#[case::its(PUPIL)]
#[case::another_creature_s(COLONY)]
#[case::those(COLONY_THOSE)]
fn g12b_counters_of_read_back(#[case] text: &str) {
    let json = serde_json::to_string(&golden(text, ClauseKind::Triggered, false)).unwrap();
    assert!(json.contains("PutCountersOf"), "{text}: {json}");
}
