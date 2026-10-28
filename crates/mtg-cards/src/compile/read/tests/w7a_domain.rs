//! Goal: domain, "the number of basic land types among lands you control", reads as a counted amount in every position (the number of, for each, where X is, conditions, an attack tax and cost changes), writes back as the card's text, and leaves each near-miss unread.
use super::*;

const W7A_YOURS: &str =
    r#"{"Distinct":{"of":[{"Type":"Land"},{"ControlledBy":"You"}],"stat":"BasicLandTypes"}}"#;

#[rstest]
#[case::tribal_flames(
    ClauseKind::Spell,
    "~ deals X damage to any target, where X is the number of basic land types among lands you control."
)]
#[case::exotic_disease(
    ClauseKind::Spell,
    "Target player loses X life and you gain X life, where X is the number of basic land types among lands you control."
)]
#[case::drag_to_the_bottom(
    ClauseKind::Spell,
    "Each creature gets -X/-X until end of turn, where X is 1 plus the number of basic land types among lands you control."
)]
#[case::territorial_maro(
    ClauseKind::Static,
    "~'s power and toughness are each equal to twice the number of basic land types among lands you control."
)]
#[case::meria_s_outrider(
    ClauseKind::Triggered,
    "When this creature enters, it deals damage to each opponent equal to the number of basic land types among lands you control."
)]
#[case::gaea_s_might(
    ClauseKind::Spell,
    "Target creature gets +1/+1 until end of turn for each basic land type among lands you control."
)]
#[case::leyline_binding(
    ClauseKind::Static,
    "This spell costs {1} less to cast for each basic land type among lands you control."
)]
#[case::jodah_s_codex(
    ClauseKind::Activated,
    "{5}, {T}: Draw a card. This ability costs {1} less to activate for each basic land type among lands you control."
)]
#[case::spore_burst(
    ClauseKind::Spell,
    "Create a 1/1 green Saproling creature token for each basic land type among lands you control."
)]
#[case::path_to_the_festival(
    ClauseKind::Spell,
    "Search your library for a basic land card, put that card onto the battlefield tapped, then shuffle. Then if there are three or more basic land types among lands you control, scry 1."
)]
#[case::collective_restraint(
    ClauseKind::Static,
    "Creatures can't attack you unless their controller pays {X} for each creature they control that's attacking you, where X is the number of basic land types among lands you control."
)]
fn w7a_domain_reads_and_writes_back(#[case] kind: ClauseKind, #[case] text: &str) {
    let json = serde_json::to_string(&golden(text, kind, false)).unwrap();
    assert!(json.contains(W7A_YOURS), "{text}: {json}");
}

#[rstest]
#[case::no_among("Draw a card for each basic land type you control.")]
#[case::singular_after_number(
    "Draw cards equal to the number of basic land type among lands you control."
)]
#[case::nonbasic("Draw a card for each nonbasic land type among lands you control.")]
#[case::condition_fewer(
    "Draw a card. Then if there are three or fewer basic land types among lands you control, scry 1."
)]
fn w7a_domain_near_misses_stay_unread(#[case] text: &str) {
    assert!(spell(text).is_none(), "{text}");
}
