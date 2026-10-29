//! Goal: the clash wordings of `w11c_clash` (a return a won clash may turn into a put on top, a hold over the clashed player's creatures) read into the DSL and write back as the card's own text; near misses stay with the table.
use super::*;

#[rstest]
#[case::whirlpool_whelm(
    "Clash with an opponent, then return target creature to its owner's hand. If you win, you may put that creature on top of its owner's library instead.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Clash":{"win":[{"May":{"effects":[{"Move":{"what":{"Target":0},"to":{"Library":{"place":"Top"}}}}]}}]}},{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#
)]
#[case::pollen_lullaby(
    "Prevent all combat damage that would be dealt this turn. Clash with an opponent. If you win, creatures that player controls don't untap during the player's next untap step.",
    r#"{"Spell":{"effects":[{"Shield":{"when":{"Damage":{"combat":true}},"by":"Prevent","duration":"UntilEndOfTurn"}},{"Clash":{"win":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":{"Bound":"clashed"}}]},"mods":["DoesntUntap"],"duration":"NextUntapStep"}}]}}]}}"#
)]
fn w11c_clash_wordings_read_and_write_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::two_targets(
    "Clash with an opponent, then return up to two target creatures to their owners' hands. If you win, you may put that creature on top of its owner's library instead."
)]
#[case::no_instead(
    "Clash with an opponent, then return target creature to its owner's hand. If you win, you may put that creature on top of its owner's library."
)]
#[case::your_creatures(
    "Clash with an opponent. If you win, creatures you control don't untap during the player's next untap step."
)]
fn w11c_clash_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
