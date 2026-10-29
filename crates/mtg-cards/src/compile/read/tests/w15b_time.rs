//! Goal: a mana value at most the time counters on the source (The Moment), a batch of cards put into exile from anywhere joined with a phase-out batch, and The War Doctor's attack damage with its exile shield read into the DSL and write back as the same text; another comparison, a zone that is not exile and counters on another object stay unread.
use super::*;

const NOT_TOKEN: &str = r#"{"Not":"Token"}"#;

#[rstest]
#[case::destroyed_by_counters(
    "{3}, {T}: Destroy each nonland permanent with mana value less than or equal to the number of time counters on ~. Then sacrifice ~. Activate only as a sorcery.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"Mana":"{3}"},"Tap"],"effects":[{"Destroy":{"what":{"Each":[{"Not":{"Type":"Land"}},{"Compare":{"stat":"ManaValue","op":"Le","to":{"Counters":{"on":"This","counter":{"Other":"time"}}}}}]}}},{"Sacrifice":{"filter":["This"]}}],"timing":"Sorcery"}}"#.to_owned()
)]
#[case::exiled_and_phased(
    "Whenever one or more other permanents phase out and whenever one or more other cards are put into exile from anywhere, put a time counter on ~.",
    ClauseKind::Triggered,
    format!(r#"{{"Triggered":{{"when":{{"AnyOf":[{{"Batch":{{"PhasedOut":{{"what":["Other"]}}}}}},{{"Batch":{{"ZoneChange":{{"what":["Other",{NOT_TOKEN}],"to":"Exile"}}}}}}]}},"effects":[{{"PutCounters":{{"on":"This","counter":{{"Other":"time"}}}}}}]}}}}"#)
)]
#[case::exiled_any_card(
    "Whenever one or more cards are put into exile from anywhere, draw a card.",
    ClauseKind::Triggered,
    format!(r#"{{"Triggered":{{"when":{{"Batch":{{"ZoneChange":{{"what":[{NOT_TOKEN}],"to":"Exile"}}}}}},"effects":[{{"Draw":{{}}}}]}}}}"#)
)]
fn w15b_time_counters_and_exile_batches_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] written: String,
) {
    reads_as(text, kind, false, &written);
}

#[test]
fn w15b_the_war_doctor_attack_reads_with_its_shield() {
    let text = "Whenever ~ attacks, it deals damage equal to the number of time counters on it to any target. If a creature dealt damage this way would die this turn, exile it instead.";
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::greater(
    "Destroy each nonland permanent with mana value greater than the number of time counters on ~ squared."
)]
#[case::graveyard_from_anywhere_of_others(
    "Whenever one or more other cards are put into a graveyard from anywhere, draw a card."
)]
#[case::exile_from_a_zone(
    "Whenever one or more other cards are put into exile from your graveyard, draw a card."
)]
fn w15b_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
