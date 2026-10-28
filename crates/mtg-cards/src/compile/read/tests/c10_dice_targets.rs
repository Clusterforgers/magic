//! Goal: "for each opponent" with up to one target that player controls (or in that player's graveyard) reads as one target specification for each opponent in a spell, a trigger and an activation and writes back as the same text; a target without "up to one", a second mention of that player and "for each player" stay unread.
use super::*;

#[rstest]
#[case::a_spell(
    "For each opponent, destroy up to one target artifact or enchantment that player controls.",
    ClauseKind::Spell,
    "Destroy"
)]
#[case::a_trigger(
    "When this creature enters, for each opponent, return up to one target nonland permanent that player controls to its owner's hand.",
    ClauseKind::Triggered,
    "Move"
)]
#[case::an_exile(
    "For each opponent, exile up to one target creature that player controls.",
    ClauseKind::Spell,
    "Exile"
)]
fn c10_dice_a_target_for_each_opponent_reads_and_writes_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] verb: &str,
) {
    let read = clause(text, kind, false).unwrap_or_else(|| panic!("not read: {text}"));
    let json = serde_json::to_string(&read).unwrap();
    assert!(
        json.contains(r#""PerPlayer":{"players":"Opponents","what":["#),
        "{json}"
    );
    assert!(json.contains(r#""bounds":{"min":0,"max":1}"#), "{json}");
    assert!(json.contains(&format!(r#"{{"{verb}":{{"#)), "{json}");
    assert!(!json.contains("RepeatEach"), "{json}");
    assert!(
        written_variants(&read)
            .iter()
            .any(|wording| same_text(wording, text)),
        "{text}: {:?}",
        written_variants(&read)
    );
}

#[rstest]
#[case::one_target_each("For each opponent, destroy target creature that player controls.")]
#[case::that_player_again(
    "For each opponent, destroy up to one target creature that player controls. That player loses 2 life."
)]
#[case::that_player_in_the_step(
    "For each opponent, destroy up to one target creature that player controls and that player discards a card."
)]
#[case::each_player("For each player, destroy up to one target creature that player controls.")]
#[case::two_targets("For each opponent, destroy up to two target creatures that player controls.")]
fn c10_dice_another_target_for_each_player_stays_unread(#[case] text: &str) {
    let read = clause(text, ClauseKind::Spell, false);
    let json = serde_json::to_string(&read).unwrap();
    assert!(!json.contains("PerPlayer"), "{text}: {json}");
}
