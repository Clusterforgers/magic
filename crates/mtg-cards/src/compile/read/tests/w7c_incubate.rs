//! Goal: incubate reads into the DSL with a number or an X, alone or after a step joined by ", then", and writes back as the card's own text; the wordings with no amount stay with the table.
use super::*;

#[rstest]
#[case::a_number(
    "{1}, {T}, Sacrifice another creature or artifact: Incubate 3.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap",{"Sacrifice":{"filter":[{"AnyOf":[{"Type":"Creature"},{"Type":"Artifact"}]},"Other"]}}],"effects":[{"Incubate":{"count":3}}]}}"#
)]
#[case::x_of_the_spell(
    "Whenever you cast a noncreature spell, incubate X, where X is that spell's mana value.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Not":{"Type":"Creature"}}],"by":"You"}},"effects":[{"Incubate":{"count":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}}]}}"#
)]
#[case::then_after_a_mill(
    "Whenever ~ enters or attacks, mill two cards, then incubate X, where X is the number of creature cards in your graveyard.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"Attacks":{"what":["This"]}}]},"effects":[{"Mill":{"count":2}},{"Incubate":{"count":{"Count":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}}]}}"#
)]
fn w7c_incubate_wordings_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::no_amount("Incubate.")]
#[case::incubate_twice("Incubate 2 twice.")]
fn w7c_incubate_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
