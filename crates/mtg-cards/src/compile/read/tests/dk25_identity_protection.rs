//! Goal: "protection from each color that's not in your commander's color identity" reads in a static grant as protection from the sources of a color outside the controller's commander identity and writes back as the same text; the same words outside a static ability, another player's commander or a near wording stay with the table.
use super::*;

#[rstest]
#[case::equipped(
    "Equipped creature gets +3/+3 and has protection from each color that's not in your commander's color identity.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":3,"toughness":3}},{"AddKeyword":{"Protection":["OutsideCommanderIdentity"]}}]}}"#
)]
fn protection_outside_the_commanders_identity_is_a_static_grant(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::as_a_spell_effect(
    "Target creature gains protection from each color that's not in your commander's color identity until end of turn.",
    ClauseKind::Spell
)]
#[case::an_opponents_commander(
    "This creature has protection from each color that's not in an opponent's commander's color identity.",
    ClauseKind::Static
)]
#[case::in_the_identity(
    "This creature has protection from each color in your commander's color identity.",
    ClauseKind::Static
)]
fn near_misses_stay_with_the_table(#[case] text: &str, #[case] kind: ClauseKind) {
    let read = clause(text, kind, false);
    let json = read
        .as_ref()
        .map(|ability| serde_json::to_string(ability).unwrap_or_default());
    assert!(
        !json.is_some_and(|json| json.contains("OutsideCommanderIdentity")),
        "{text}: {read:?}"
    );
}
