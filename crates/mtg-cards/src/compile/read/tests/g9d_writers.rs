//! Goal: the writers of round 9 (g9d) give the oracle wording of a look at the top cards put back with a shuffle the controller may have, of target player's library or your own, alone or before a draw, and of a shuffle-in of up to some target cards of your graveyard; a wrong player, count, order, bound, owner or missing "may" never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const SELECTION: &str = r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"LookAtTop":{"who":{"Target":0},"count":3,"as":"top"}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Top","order":"Chosen"}}}},{"May":{"effects":[{"Shuffle":{"who":{"Target":0}}}]}}]}}"#;
const MAGE: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"LookAtTop":{"count":3,"as":"top"}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Top","order":"Chosen"}}}},{"May":{"effects":[{"Shuffle":{}}]}}]}}"#;
const OMEN: &str = r#"{"Spell":{"effects":[{"LookAtTop":{"count":3,"as":"top"}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Top","order":"Chosen"}}}},{"May":{"effects":[{"Shuffle":{}}]}},{"Draw":{}}]}}"#;
const CASTAWAY: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"OwnedBy":"You"},{"In":"Graveyard"}]},"bounds":{"min":0,"max":3}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Library":{"place":"Shuffled"}}}}]}}"#;

const NATURAL: &str = "Look at the top three cards of target player's library, then put them back in any order. You may have that player shuffle.";
const PONDERING: &str = "When this creature enters, look at the top three cards of your library, then put them back in any order. You may shuffle.";
const PONDER: &str = "Look at the top three cards of your library, then put them back in any order. You may shuffle.\nDraw a card.";
const SHUFFLE_IN: &str = "When this creature enters, you may shuffle up to three target cards from your graveyard into your library.";

#[rstest]
#[case::target_player(SELECTION, NATURAL)]
#[case::your_library(MAGE, PONDERING)]
#[case::before_a_draw(OMEN, PONDER)]
#[case::shuffle_in_up_to(CASTAWAY, SHUFFLE_IN)]
fn the_writers_give_the_oracle_wording(#[case] json: &str, #[case] clause: &str) {
    assert!(
        written(json, clause),
        "{:?}",
        written_variants(&ability(json))
    );
}

#[rstest]
// The shuffle of another library than the one looked at.
#[case::other_shuffler(SELECTION.replace(r#"{"Shuffle":{"who":{"Target":0}}}"#, r#"{"Shuffle":{}}"#), NATURAL)]
// The shuffle made without a choice.
#[case::no_may(SELECTION.replace(r#"{"May":{"effects":[{"Shuffle":{"who":{"Target":0}}}]}}"#, r#"{"Shuffle":{"who":{"Target":0}}}"#), NATURAL)]
// The target player decides.
#[case::other_decider(SELECTION.replace(r#"{"May":{"effects""#, r#"{"May":{"who":{"Target":0},"effects""#), NATURAL)]
// Two cards, not three.
#[case::other_count(MAGE.replace(r#""count":3"#, r#""count":2"#), PONDERING)]
// A random order.
#[case::random_order(MAGE.replace("Chosen", "Random"), PONDERING)]
// Put on the bottom.
#[case::bottom(MAGE.replace(r#""place":"Top""#, r#""place":"Bottom""#), PONDERING)]
// Two cards drawn.
#[case::two_draws(OMEN.replace(r#"{"Draw":{}}"#, r#"{"Draw":{"count":2}}"#), PONDER)]
// Exactly three, so not "you may ... up to".
#[case::exactly(CASTAWAY.replace(r#""bounds":{"min":0,"max":3}"#, r#""bounds":{"min":3,"max":3}"#), SHUFFLE_IN)]
// Cards of any graveyard go to their owners' libraries.
#[case::any_owner(CASTAWAY.replace(r#"{"OwnedBy":"You"},"#, ""), SHUFFLE_IN)]
// Put on top, not shuffled in.
#[case::on_top(CASTAWAY.replace(r#"{"place":"Shuffled"}"#, r#"{"place":"Top"}"#), SHUFFLE_IN)]
fn a_wrong_rule_never_reads_back(#[case] json: String, #[case] clause: &str) {
    assert!(
        !written(&json, clause),
        "{:?}",
        written_variants(&ability(&json))
    );
}
