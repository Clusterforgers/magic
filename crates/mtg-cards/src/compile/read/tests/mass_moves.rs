//! Goal: a mass move reads with every object the phrase names (to the bottom or top of the owners' libraries, to the owners' hands, from your graveyard to the battlefield), a hand discarded by each player or opponent reads as the cards in those hands, each writes back as the card's own text, and any other place, owner or order stays with the table.
use super::*;

#[rstest]
#[case::terminus(
    "Put all creatures on the bottom of their owners' libraries.",
    r#"{"Spell":{"effects":[{"Move":{"what":{"Each":[{"Type":"Creature"}]},"to":{"Library":{"place":"Bottom"}}}}]}}"#
)]
#[case::on_top(
    "Put all creatures on top of their owners' libraries.",
    r#"{"Spell":{"effects":[{"Move":{"what":{"Each":[{"Type":"Creature"}]},"to":{"Library":{"place":"Top"}}}}]}}"#
)]
#[case::devastation_tide(
    "Return all nonland permanents to their owners' hands.",
    r#"{"Spell":{"effects":[{"Move":{"what":{"Each":[{"Not":{"Type":"Land"}}]},"to":"Hand"}}]}}"#
)]
fn a_move_of_every_permanent_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::redress_fate(
    "Return all artifact and enchantment cards from your graveyard to the battlefield."
)]
#[case::creatures("Return all creature cards from your graveyard to the battlefield.")]
#[case::lands("Return all land cards from your graveyard to the battlefield.")]
#[case::bounce_creatures("Return all creatures to their owners' hands.")]
#[case::bounce_yours("Return all creatures you control to their owners' hands.")]
#[case::bounce_artifacts("Return all artifacts to their owners' hands.")]
#[case::bottom_artifacts("Put all artifacts on the bottom of their owners' libraries.")]
#[case::after_a_sentence("Draw a card. Return all nonland permanents to their owners' hands.")]
fn more_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case::each_player(
    "Each player discards their hand, then draws seven cards.",
    r#"{"Spell":{"effects":[{"Discard":{"who":"EachPlayer","count":{"Count":[{"In":"Hand"},{"OwnedBy":"EachPlayer"}]}}},{"Draw":{"who":"EachPlayer","count":7}}]}}"#
)]
#[case::each_player_alone(
    "Each player discards their hand.",
    r#"{"Spell":{"effects":[{"Discard":{"who":"EachPlayer","count":{"Count":[{"In":"Hand"},{"OwnedBy":"EachPlayer"}]}}}]}}"#
)]
#[case::each_opponent(
    "Each opponent discards their hand.",
    r#"{"Spell":{"effects":[{"Discard":{"who":"Opponents","count":{"Count":[{"In":"Hand"},{"OwnedBy":"Opponents"}]}}}]}}"#
)]
fn a_hand_discarded_by_several_players_reads_as_the_cards_in_their_hands(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::their_owner("Put all creatures on the bottom of their owner's library.")]
#[case::your_library("Put all creatures on the bottom of your library.")]
#[case::any_order("Put all creatures on the bottom of their owners' libraries in any order.")]
#[case::graveyard_to_hands("Return all creature cards from your graveyard to their owners' hands.")]
#[case::all_graveyards("Return all creature cards from all graveyards to the battlefield.")]
#[case::permanents_to_the_battlefield("Return all creatures to the battlefield.")]
#[case::half("Return half of the creatures to their owners' hands.")]
fn near_misses_of_a_mass_move_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
