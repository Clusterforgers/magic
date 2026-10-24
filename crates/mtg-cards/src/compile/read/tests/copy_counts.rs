//! Goal: a token copy whose number is named by a where clause that sits between the copy and the sentences that act on it ("create X tokens that are copies of ..., where X is ... They gain haste. Exile them ...") reads into one `CreateToken` of `CopyOf` with the counted amount and the bound sentences after it, writes back as the same text, and leaves a where clause that names no count or a count that no sentence needs to the table.
use super::*;

#[rstest]
#[case::counted_spells_then_haste_and_exile(
    "At the beginning of combat on your turn, create X tokens that are copies of another target creature you control, where X is one plus the number of instant and sorcery spells you've cast this turn. They gain haste. Exile them at the beginning of the next end step.",
    ClauseKind::Triggered,
    r#""as":"copy""#
)]
#[case::copies_of_itself_attacking_then_exile(
    "Whenever ~ attacks, create X tokens that are copies of it and that are tapped and attacking, where X is the number of creatures defending player controls. Exile the tokens at the beginning of the next end step.",
    ClauseKind::Triggered,
    r#""attacking":true"#
)]
#[case::a_spell_counts_creatures(
    "Create X tokens that are copies of target creature you control, where X is the number of creatures you control. Those tokens gain haste. Sacrifice them at end of combat.",
    ClauseKind::Spell,
    r#""step":"EndCombat""#
)]
#[case::a_token_for_each_discard_that_enters_attacking(
    "Whenever ~ attacks, for each card you've discarded this turn, create a token that's a copy of ~, except the token isn't legendary. The tokens enter tapped and attacking. Exile the tokens at the beginning of the next end step.",
    ClauseKind::Triggered,
    r#""what":"Discarded""#
)]
#[case::a_token_for_each_death_alone(
    "At the beginning of your end step, for each creature put into your graveyard from the battlefield this turn, create a token that's a copy of ~.",
    ClauseKind::Triggered,
    r#""what":"CreaturesDied""#
)]
#[case::one_token_that_enters_attacking(
    "Whenever ~ attacks, create a token that's a copy of it. The token enters tapped and attacking.",
    ClauseKind::Triggered,
    r#""tapped":true,"attacking":true"#
)]
fn a_number_of_copies_counted_before_the_sentences_that_act_on_them(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] part: &str,
) {
    let read = golden(text, kind, false);
    let json = serde_json::to_string(&read).unwrap_or_default();
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::a_tapped_token_for_each(
    "Whenever ~ attacks, for each card you've discarded this turn, create a tapped token that's a copy of ~."
)]
#[case::the_cards_an_opponent_discarded(
    "Whenever ~ attacks, for each card an opponent discarded this turn, create a token that's a copy of ~."
)]
#[case::the_wrong_number_of_tokens_enter(
    "Whenever ~ attacks, for each card you've discarded this turn, create a token that's a copy of ~. The token enters tapped and attacking."
)]
#[case::the_tokens_enter_tapped_only(
    "Whenever ~ attacks, for each card you've discarded this turn, create a token that's a copy of ~. The tokens enter tapped."
)]
#[case::one_token_in_the_plural(
    "Whenever ~ attacks, create a token that's a copy of it. The tokens enter tapped and attacking."
)]
#[case::a_where_clause_that_names_no_count(
    "Whenever ~ attacks, create X tokens that are copies of it, where X is the colors of ~. They gain haste."
)]
fn a_count_or_an_entry_that_does_not_match_stays_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
