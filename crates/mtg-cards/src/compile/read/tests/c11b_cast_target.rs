//! Goal: the cast of a target card during a resolution, for nothing or for its mana cost, and the exile of that spell
//! instead of a graveyard read in a spell and a trigger and write back as the same text; a near-miss (another verb,
//! another pronoun, another zone, the exile without a cast before it) stays unread.
use super::*;

#[rstest]
#[case::gearhulk(
    "When this creature enters, you may cast target instant card from your graveyard without paying its mana cost. If that spell would be put into your graveyard, exile it instead.",
    ClauseKind::Triggered,
    true
)]
#[case::dark_dwellers(
    "When this creature enters, you may cast target instant or sorcery card with mana value 3 or less from your graveyard without paying its mana cost. If that spell would be put into your graveyard, exile it instead.",
    ClauseKind::Triggered,
    true
)]
#[case::any_graveyard(
    "When this creature enters, you may cast target instant or sorcery card from a graveyard without paying its mana cost. If that spell would be put into a graveyard, exile it instead.",
    ClauseKind::Triggered,
    true
)]
#[case::an_opponents_graveyard(
    "You may cast target instant or sorcery card from an opponent's graveyard without paying its mana cost. If that spell would be put into their graveyard, exile it instead.",
    ClauseKind::Spell,
    true
)]
#[case::no_exile(
    "When this creature enters, you may cast target sorcery card from your graveyard without paying its mana cost.",
    ClauseKind::Triggered,
    false
)]
#[case::that_players_graveyard(
    "Whenever this creature deals combat damage to a player, you may cast target instant or sorcery card from that player's graveyard without paying its mana cost. If that spell would be put into a graveyard, exile it instead.",
    ClauseKind::Triggered,
    true
)]
#[case::paid(
    "When this creature enters, you may cast target instant card from your graveyard. If that spell would be put into your graveyard, exile it instead.",
    ClauseKind::Triggered,
    true
)]
fn c11b_cast_target_reads_and_writes_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] exiled: bool,
) {
    let read = golden(text, kind, false);
    let json = serde_json::to_string(&read).unwrap();
    assert!(json.contains(r#""CastFromZone":{"what":{"Target":0}"#), "{json}");
    assert_eq!(json.contains(r#""MoveTo":"Exile""#), exiled, "{json}");
}

#[rstest]
#[case::play("When this creature enters, you may play target land card from your graveyard.")]
#[case::other_pronoun(
    "When this creature enters, you may cast target instant card from your graveyard without paying its mana cost. If that card would be put into your graveyard, exile it instead."
)]
#[case::other_zone(
    "When this creature enters, you may cast target instant card from your graveyard without paying its mana cost. If that spell would be put into your hand, exile it instead."
)]
#[case::no_cast_before("If that spell would be put into your graveyard, exile it instead.")]
fn c11b_cast_target_near_misses_stay_unread(#[case] text: &str) {
    let kind = if text.starts_with("When") {
        ClauseKind::Triggered
    } else {
        ClauseKind::Spell
    };
    let read = clause(text, kind, false);
    let written = read.as_ref().map(render_variants).unwrap_or_default();
    assert!(
        !written.iter().any(|w| same_text(w, text)),
        "{text}: {read:?}"
    );
}
