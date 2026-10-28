//! Goal: "whenever you expend N" reads into the DSL with the number it names, before the effects of the card, and writes back as the card's own text; a word for the number, a zero and a missing number stay with the table.
use super::*;

#[rstest]
#[case::a_counter(
    "Whenever you expend 4, put a +1/+1 counter on ~.",
    r#"{"Triggered":{"when":{"Expend":{"who":"You","n":4}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#
)]
#[case::damage_to_each_opponent(
    "Whenever you expend 4, ~ deals 2 damage to each opponent.",
    r#"{"Triggered":{"when":{"Expend":{"who":"You","n":4}},"effects":[{"DealDamage":{"to":"Opponents","amount":2}}]}}"#
)]
#[case::a_permanent_card_back(
    "Whenever you expend 8, return up to one target permanent card from your graveyard to your hand.",
    r#"{"Triggered":{"when":{"Expend":{"who":"You","n":8}},"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]},{"In":"Graveyard"},{"OwnedBy":"You"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#
)]
#[case::control_of_an_artifact(
    "Whenever you expend 6, gain control of up to one target artifact for as long as you control ~.",
    r#"{"Triggered":{"when":{"Expend":{"who":"You","n":6}},"targets":[{"what":{"Object":[{"Type":"Artifact"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"}],"duration":"WhileYouControlSource"}}]}}"#
)]
fn w9a_expend_wordings_read_and_write_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::a_word_for_the_number("Whenever you expend four, put a +1/+1 counter on ~.")]
#[case::zero("Whenever you expend 0, put a +1/+1 counter on ~.")]
#[case::no_number("Whenever you expend, put a +1/+1 counter on ~.")]
fn w9a_expend_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
