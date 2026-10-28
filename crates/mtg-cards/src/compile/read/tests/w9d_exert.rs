//! Goal: exert as a creature attacks reads into the keyword (also once per turn), "When you do" after it into a trigger on the card being exerted whose "it" is the card, and "Whenever you exert a creature" into the same trigger on the creatures its controller controls; each writes back as the card's own text, and the near misses stay with the table.
use super::*;

#[rstest]
#[case::exert(
    "You may exert ~ as it attacks.",
    ClauseKind::Static,
    r#"{"Keyword":{"Exert":{}}}"#
)]
#[case::exert_this_creature(
    "You may exert this creature as it attacks.",
    ClauseKind::Static,
    r#"{"Keyword":{"Exert":{}}}"#
)]
#[case::exert_he(
    "You may exert ~ as he attacks.",
    ClauseKind::Static,
    r#"{"Keyword":{"Exert":{}}}"#
)]
#[case::once_per_turn(
    "If this creature hasn't been exerted this turn, you may exert it as it attacks.",
    ClauseKind::Static,
    r#"{"Keyword":{"Exert":{"once_per_turn":true}}}"#
)]
#[case::damage(
    "When you do, it deals 4 damage to target non-Dragon creature an opponent controls.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Exerted":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"Not":{"Subtype":"Dragon"}},{"ControlledBy":"Opponents"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":4}}]}}"#
)]
#[case::pump("When you do, it gets +1/+2 until end of turn.", ClauseKind::Triggered, r#"{"Triggered":{"when":{"Exerted":{"what":["This"]}},"effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":2}}],"duration":"UntilEndOfTurn"}}]}}"#)]
#[case::draw(
    "When you do, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Exerted":{"what":["This"]}},"effects":[{"Draw":{}}]}}"#
)]
#[case::untap_others(
    "When you do, untap all other creatures you control.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Exerted":{"what":["This"]}},"effects":[{"Untap":{"what":{"Each":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]}}}]}}"#
)]
#[case::whenever_you_exert(
    "Whenever you exert a creature, tap target creature an opponent controls.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Exerted":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Tap":{"what":{"Target":0}}}]}}"#
)]
fn w9d_exert_wordings_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::exert_later("You may exert ~ as it blocks.")]
#[case::another_creature("You may exert target creature as it attacks.")]
#[case::not_this_turn("If ~ has been exerted this turn, you may exert it as it attacks.")]
fn w9d_exert_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
