//! Goal: the nodes of "empower Jace N" and "you may behold a Jace. If you don't, ~ enters tapped" write back as printed,
//! and a near miss (another planeswalker, a reveal, no number) is not read as them.
use super::*;

#[rstest]
#[case::fixed("When ~ enters, empower Jace 5.", ClauseKind::Triggered, r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Empower":{"count":5}}]}}"#)]
#[case::activated(
    "{2}{U}, {T}: Empower Jace 2.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"Mana":"{2}{U}"},"Tap"],"effects":[{"Empower":{"count":2}}]}}"#
)]
#[case::behold("As ~ enters, you may behold a Jace. If you don't, ~ enters tapped.", ClauseKind::Static, r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"BeholdOrTapped":[{"Subtype":"Jace"}]}}}"#)]
fn r5508_writes_back(#[case] text: &str, #[case] kind: ClauseKind, #[case] json: &str) {
    let _ = kind;
    let written = render_variants(&ability(json));
    assert!(
        written.iter().any(|wording| same_text(wording, text)),
        "{text}: {written:?}"
    );
}

#[rstest]
#[case::another_walker("When ~ enters, empower Chandra 5.", ClauseKind::Triggered)]
#[case::no_number("When ~ enters, empower Jace.", ClauseKind::Triggered)]
#[case::behold_from_hand(
    "As ~ enters, you may behold a Jace card from your hand. If you don't, ~ enters tapped.",
    ClauseKind::Static
)]
fn r5508_a_near_miss_is_not_read(#[case] text: &str, #[case] kind: ClauseKind) {
    let read = clause(text, kind, false);
    assert!(
        !read.iter().any(|ability| {
            let json = serde_json::to_string(ability).unwrap_or_default();
            json.contains("Empower") || json.contains("BeholdOrTapped")
        }),
        "{text}: {read:?}"
    );
}
