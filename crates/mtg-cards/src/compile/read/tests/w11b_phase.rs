//! Goal: phase-out durations and counts ("until ~ leaves the battlefield", "Tap that creature as it phases in this way", "untap all creatures, then those creatures phase out ...", "It phases out ..." after a target, "for each creature that phased out this way") and the phase-out and phase-in triggers read into the DSL and write back as the same text; another duration, a tap without the duration, a group that changes and a wrong verb number stay unread.
use super::*;

const ENTERS: &str = r#""when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}}"#;
const UNTIL: &str = r#""duration":"WhileSourceOnBattlefield""#;

#[rstest]
#[case::until_and_tapped(
    "When ~ enters, target creature phases out until ~ leaves the battlefield. Tap that creature as it phases in this way.",
    ClauseKind::Triggered,
    format!(r#"{{"Triggered":{{{ENTERS},"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"PhaseOut":{{"what":{{"Target":0}},{UNTIL},"tapped":true}}}}]}}}}"#)
)]
#[case::until_a_permanent(
    "Target permanent phases out until ~ leaves the battlefield.",
    ClauseKind::Spell,
    format!(r#"{{"Spell":{{"targets":[{{"what":{{"Object":[]}}}}],"effects":[{{"PhaseOut":{{"what":{{"Target":0}},{UNTIL}}}}}]}}}}"#)
)]
#[case::untapped_then_out(
    "When ~ enters, untap all creatures, then those creatures phase out until ~ leaves the battlefield.",
    ClauseKind::Triggered,
    format!(r#"{{"Triggered":{{{ENTERS},"effects":[{{"Untap":{{"what":{{"Each":[{{"Type":"Creature"}}]}}}}}},{{"PhaseOut":{{"what":{{"Each":[{{"Type":"Creature"}}]}},{UNTIL}}}}}]}}}}"#)
)]
#[case::counted(
    "Put a time counter on ~ for each creature that phased out this way.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"PutCounters":{"on":"This","counter":{"Other":"time"},"count":{"PhasedOut":[{"Type":"Creature"}]}}}]}}"#.to_owned()
)]
#[case::it_after_a_target(
    "{2}, {T}: Untap target creature you control. It phases out until ~ leaves the battlefield.",
    ClauseKind::Activated,
    format!(r#"{{"Activated":{{"cost":[{{"Mana":"{{2}}"}},"Tap"],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}}}}],"effects":[{{"Untap":{{"what":{{"Target":0}}}}}},{{"PhaseOut":{{"what":{{"Target":0}},{UNTIL}}}}}]}}}}"#)
)]
#[case::itself_out(
    "Whenever ~ phases out, discard a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"PhasedOut":{"what":["This"]}},"effects":[{"Discard":{}}]}}"#.to_owned()
)]
#[case::itself_in(
    "Whenever ~ phases in, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"PhasedIn":{"what":["This"]}},"effects":[{"Draw":{}}]}}"#.to_owned()
)]
#[case::yours_in(
    "Whenever a creature you control phases in, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"PhasedIn":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"effects":[{"Draw":{}}]}}"#.to_owned()
)]
#[case::one_or_more_out(
    "Whenever one or more other permanents phase out, put a time counter on ~.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Batch":{"PhasedOut":{"what":["Other"]}}},"effects":[{"PutCounters":{"on":"This","counter":{"Other":"time"}}}]}}"#.to_owned()
)]
fn w11b_phase_durations_and_triggers_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] written: String,
) {
    reads_as(text, kind, false, &written);
}

#[rstest]
#[case::another_duration("Target creature phases out until end of turn.")]
#[case::tapped_without_the_duration(
    "Target creature phases out. Tap that creature as it phases in this way."
)]
#[case::tapped_for_a_group(
    "All creatures phase out until ~ leaves the battlefield. Tap that creature as it phases in this way."
)]
#[case::another_group(
    "Untap all creatures, then those artifacts phase out until ~ leaves the battlefield."
)]
#[case::untapped_without_the_duration("Untap all creatures, then those creatures phase out.")]
#[case::plural_verb_for_one("Target creature phase out until ~ leaves the battlefield.")]
#[case::another_permanent_leaves(
    "Target creature phases out until that creature leaves the battlefield."
)]
fn w11b_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
