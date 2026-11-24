//! Goal: "put up to N <kind> counters on <place>" reads as a number chosen from none to N and that many counters put, in
//! a spell, a trigger and an activated ability, and writes back as one sentence; the plain put of N is not written with
//! "up to", and the choice and put do not read as the plain put.
use super::*;

fn up_to(on: &str, kind: &str, most: u32) -> String {
    format!(
        r#"{{"ChooseValue":{{"kind":{{"Number":[0,{most}]}},"as":"up_to"}}}},{{"PutCounters":{{"on":{on},"counter":"{kind}","count":{{"ChosenAs":"up_to"}}}}}}"#
    )
}

#[rstest]
#[case::a_spell(
    "Put up to two +1/+1 counters on target creature.",
    ClauseKind::Spell,
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{}]}}}}"#,
        up_to(r#"{"Target":0}"#, "+1/+1", 2)
    )
)]
#[case::a_trigger(
    "When this creature enters, put up to three +1/+1 counters on it.",
    ClauseKind::Triggered,
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{}]}}}}"#,
        up_to(r#""This""#, "+1/+1", 3)
    )
)]
#[case::an_activation(
    "{T}: Put up to two charge counters on ~.",
    ClauseKind::Activated,
    format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{}]}}}}"#,
        up_to(r#""This""#, "charge", 2)
    )
)]
fn r121_up_to_counters_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: String,
) {
    assert_eq!(golden(text, kind, false), ability(&json));
}

#[rstest]
#[case::the_plain_put(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":2}}]}}"#,
    "Put up to two +1/+1 counters on target creature."
)]
#[case::from_one(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"ChooseValue":{"kind":{"Number":[1,2]},"as":"up_to"}},{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":{"ChosenAs":"up_to"}}}]}}"#,
    "Put up to two +1/+1 counters on target creature."
)]
#[case::the_most_put(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"ChooseValue":{"kind":{"Number":[0,2]},"as":"up_to"}},{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":{"ChosenAs":"up_to"}}}]}}"#,
    "Put two +1/+1 counters on target creature."
)]
fn r121_up_to_near_misses_differ(#[case] json: &str, #[case] text: &str) {
    let found = written_variants(&ability(json));
    assert!(!found.iter().any(|w| same_text(w, text)), "{found:?}");
}
