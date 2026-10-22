//! Goal: "You may cast <spells> from the top of your library." reads as a cast permission from the library over the cards the spell group names (a card type, a subtype, a type with "non", two joined by "and"), writes back as the same text, and leaves a rider, another zone, an owner and every phrase that is not a spell group to the table.
use super::*;

#[rstest]
#[case::creatures(
    "You may cast creature spells from the top of your library.",
    r#"{"Rule":{"rule":{"MayCast":{"what":[{"Type":"Creature"}],"from":"Library"}}}}"#
)]
#[case::noncreature(
    "You may cast noncreature spells from the top of your library.",
    r#"{"Rule":{"rule":{"MayCast":{"what":[{"Not":{"Type":"Creature"}}],"from":"Library"}}}}"#
)]
#[case::a_subtype(
    "You may cast Merfolk spells from the top of your library.",
    r#"{"Rule":{"rule":{"MayCast":{"what":[{"Subtype":"Merfolk"}],"from":"Library"}}}}"#
)]
fn a_cast_from_the_top_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::artifacts("You may cast artifact spells from the top of your library.")]
#[case::goblins("You may cast Goblin spells from the top of your library.")]
#[case::enchantments("You may cast enchantment spells from the top of your library.")]
#[case::two_types("You may cast instant and sorcery spells from the top of your library.")]
fn more_oracle_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Static, false);
}

#[rstest]
#[case::every_spell("You may cast spells from the top of your library.")]
#[case::a_rider(
    "You may cast noncreature spells from the top of your library. If you cast a spell this way, you may cast it as though it had flash."
)]
#[case::the_graveyard("You may cast creature spells from your graveyard.")]
#[case::a_hand("You may cast creature spells from your hand.")]
#[case::an_opponent("You may cast creature spells from the top of target opponent's library.")]
#[case::a_land_too("You may play lands and cast spells from the top of your library.")]
#[case::a_cost("You may cast creature spells from the top of your library by paying life.")]
#[case::not_a_group("You may cast Fireball from the top of your library.")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
