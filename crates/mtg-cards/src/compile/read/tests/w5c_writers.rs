//! Goal: the writers of the step leftovers give the oracle wording of a delayed trigger said after its effects and of the counters a player gets, and a changed clause does not read back.
use super::*;

/// The wordings the checker accepts for the rules, for the message of a failed golden.
fn wordings(json: &str) -> Vec<String> {
    written_variants(&ability(json))
}

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    wordings(json)
        .iter()
        .any(|wording| same_text(wording, clause))
}

const OHRAN: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":{"Each":[{"Type":"Creature"}]},"combat":true}},"effects":[{"DelayedTrigger":{"when":{"StepBegins":{"step":"EndCombat"}},"effects":[{"Destroy":{"what":"Triggering"}}]}}]}}"#;

const RADGULL: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"If":{"condition":{"Compare":{"left":{"Counters":{"on":"Triggering","counter":{"Other":"rad"}}},"op":"Eq","right":0}},"then":[{"PutCounters":{"on":"Triggering","counter":{"Other":"rad"},"count":2}}],"otherwise":[{"Proliferate":{}}]}}]}}"#;
const MEREN: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"PutCounters":{"on":"You","counter":{"Other":"experience"}}}]}}"#;
const ON_ITSELF: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"PutCounters":{"on":"This","counter":{"Other":"experience"}}}]}}"#;

#[rstest]
#[case::radgull(
    RADGULL,
    "Whenever this creature deals combat damage to a player, that player gets two rad counters if they don't have any rad counters. Otherwise, proliferate."
)]
#[case::meren(
    MEREN,
    "Whenever another creature you control dies, you get an experience counter."
)]
#[case::ohran(
    OHRAN,
    "Whenever ~ deals combat damage to a creature, destroy that creature at end of combat."
)]
fn writes_the_oracle_wording(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}: {:?}", wordings(json));
}

#[rstest]
#[case::radgull_count(
    RADGULL,
    "Whenever this creature deals combat damage to a player, that player gets three rad counters if they don't have any rad counters. Otherwise, proliferate."
)]
#[case::radgull_kind(
    RADGULL,
    "Whenever this creature deals combat damage to a player, that player gets two poison counters if they don't have any rad counters. Otherwise, proliferate."
)]
#[case::radgull_player(
    RADGULL,
    "Whenever this creature deals combat damage to a player, you get two rad counters if they don't have any rad counters. Otherwise, proliferate."
)]
#[case::meren_player(
    MEREN,
    "Whenever another creature you control dies, that player gets an experience counter."
)]
#[case::meren_count(
    MEREN,
    "Whenever another creature you control dies, you get two experience counters."
)]
#[case::on_itself(
    ON_ITSELF,
    "Whenever another creature you control dies, you get an experience counter."
)]
#[case::ohran_step(
    OHRAN,
    "Whenever ~ deals combat damage to a creature, destroy that creature at the beginning of the next end step."
)]
#[case::ohran_kind(
    OHRAN,
    "Whenever ~ deals combat damage to an artifact, destroy that creature at end of combat."
)]
fn a_changed_clause_does_not_read_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}
