//! Goal: a card type in front of another in the singular ("target artifact creature") and a token copy that enters "tapped and attacking" (in either order of the words, also "copies of it" in a trigger of the card's own attack, and a count X named by the sentence's where clause) read into `CreateToken` of `CopyOf` with `tapped` and `attacking`, write back as the same text, and every untapped or plural near-miss stays with the table.
use super::*;

#[rstest]
#[case::artifact_creature_target(
    "Choose target artifact creature you control. For each creature chosen this way, create a token that's a copy of it.",
    ClauseKind::Spell,
    r#"{"Type":"Artifact"},{"Type":"Creature"}"#
)]
#[case::destroy_an_artifact_creature(
    "Destroy target artifact creature.",
    ClauseKind::Spell,
    r#"{"Type":"Artifact"},{"Type":"Creature"}"#
)]
#[case::attacking_creature_and_tapped(
    "Whenever this creature attacks, create a token that's a copy of another target attacking creature and that's tapped and attacking. Exile the token at end of combat.",
    ClauseKind::Triggered,
    r#""tapped":true,"attacking":true"#
)]
#[case::tapped_and_attacking_token_with_exceptions(
    "Whenever ~ attacks, create a tapped and attacking token that's a copy of another target Villain you control, except it isn't legendary and it's an Illusion in addition to its other types. Sacrifice that token at the beginning of the next end step.",
    ClauseKind::Triggered,
    r#""tapped":true,"attacking":true"#
)]
#[case::copies_of_itself_attacking(
    "Whenever ~ attacks, create two tokens that are copies of it and that are tapped and attacking.",
    ClauseKind::Triggered,
    r#""CopyOf":{"of":"This""#
)]
#[case::copies_of_itself_by_a_where_clause(
    "Whenever ~ attacks, create X tokens that are copies of it and that are tapped and attacking, where X is the number of creatures defending player controls.",
    ClauseKind::Triggered,
    r#""attacking":true"#
)]
fn a_card_type_qualifier_and_a_tapped_and_attacking_copy(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] part: &str,
) {
    let read = golden(text, kind, false);
    let json = serde_json::to_string(&read).unwrap_or_default();
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::attacking_untapped(
    "Whenever ~ attacks, create a token that's a copy of another target attacking creature and that's attacking.",
    ClauseKind::Triggered
)]
#[case::the_tapped_and_attacking_tokens(
    "Whenever ~ attacks, create two tapped and attacking tokens that are copies of it.",
    ClauseKind::Triggered
)]
#[case::an_attacking_token_of_two_targets(
    "Create a token that's a copy of each of two target creatures and that's tapped and attacking.",
    ClauseKind::Spell
)]
#[case::an_artifact_land_is_no_pair_of_nouns(
    "Choose target artifact land you control. For each land chosen this way, create a token that's a copy of it.",
    ClauseKind::Spell
)]
#[case::the_wrong_noun_for_an_artifact_creature(
    "Choose target artifact creature you control. For each land chosen this way, create a token that's a copy of it.",
    ClauseKind::Spell
)]
fn what_is_not_the_shape_of_an_attacking_copy_stays_with_the_table(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
