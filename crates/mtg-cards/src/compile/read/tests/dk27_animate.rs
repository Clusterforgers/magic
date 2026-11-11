//! Goal: Animate Dead's trigger reads as the return of the enchanted card, the attachment to it and the delayed
//! sacrifice, writes back as the printed text, and a near miss of the text or of the rules reads or writes as nothing.
use super::*;

const TEXT: &str = "When ~ enters, if it's on the battlefield, it loses \"enchant creature card in a graveyard\" and gains \"enchant creature put onto the battlefield with ~.\" Return enchanted creature card to the battlefield under your control and attach ~ to it. When ~ leaves the battlefield, that creature's controller sacrifices it.";

const RULES: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"Matches":{"subject":"This","selector":[{"In":"Battlefield"}]}},"effects":[{"Move":{"what":"Attached","to":{"Battlefield":{"under":"You"}},"as":"returned"}},{"Attach":{"what":"This","to":{"Bound":"returned"}}},{"DelayedTrigger":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield"}},"effects":[{"Sacrifice":{"who":{"ControllerOf":{"Bound":"returned"}},"filter":[{"Is":{"Bound":"returned"}}]}}]}}]}}"#;

#[test]
fn animate_dead_reads_and_writes_back() {
    reads_as(TEXT, ClauseKind::Triggered, false, RULES);
}

#[rstest]
#[case::owner_control("under your control", "under its owner's control")]
#[case::no_sacrifice("that creature's controller sacrifices it", "you draw a card")]
#[case::exile_instead("sacrifices it", "exiles it")]
fn a_near_miss_of_the_text_stays_unread(#[case] from: &str, #[case] to: &str) {
    let text = TEXT.replace(from, to);
    assert_eq!(clause(&text, ClauseKind::Triggered, false), None, "{text}");
}

#[rstest]
#[case::under_its_owner(
    r#""to":{"Battlefield":{"under":"You"}}"#,
    r#""to":{"Battlefield":{}}"#
)]
#[case::no_condition(
    r#""if":{"Matches":{"subject":"This","selector":[{"In":"Battlefield"}]}},"#,
    ""
)]
#[case::destroyed(r#"{"Sacrifice":{"who":{"ControllerOf":{"Bound":"returned"}},"filter":[{"Is":{"Bound":"returned"}}]}}"#, r#"{"Destroy":{"what":{"Bound":"returned"}}}"#)]
fn a_near_miss_of_the_rules_is_not_written_as_the_text(#[case] from: &str, #[case] to: &str) {
    let json = RULES.replacen(from, to, 1);
    assert_ne!(json, RULES, "{from}");
    let written = render_variants(&ability(&json));
    assert!(
        !written.iter().any(|said| same_text(said, TEXT)),
        "{json}: {written:?}"
    );
}
