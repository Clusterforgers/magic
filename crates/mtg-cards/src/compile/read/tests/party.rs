//! Goal: the party reads in the three places cards use it ("the number of creatures in your party" and "for each creature in your party" as an amount, "if you have a full party" as a condition) and "protection from the chosen player" reads in a static ability only; each writes back as the same text, and the party of another player and the other readings of "party" stay with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::life_for_each(
    "When this creature enters, you gain 2 life for each creature in your party.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"GainLife":{"amount":{"Times":{"by":2,"of":{"Party":"You"}}}}}]}}"#
)]
#[case::where_x_is_the_number(
    "When this creature enters, each opponent loses X life and you gain X life, where X is the number of creatures in your party.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"LoseLife":{"who":"Opponents","amount":{"Party":"You"}}},{"GainLife":{"amount":{"Party":"You"}}}]}}"#
)]
#[case::a_full_party_in_an_intervening_if(
    "At the beginning of combat on your turn, if you have a full party, draw a card.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"if":{"Compare":{"left":{"Party":"You"},"op":"Ge","right":4}},"effects":[{"Draw":{}}]}}"#
)]
fn the_party_is_read_as_an_amount_and_a_condition(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::another_players_party(
    "When this creature enters, you gain 2 life for each creature in their party."
)]
#[case::an_opponents_party(
    "When this creature enters, you gain life equal to the number of creatures in an opponent's party."
)]
#[case::a_half_party(
    "At the beginning of combat on your turn, if you have a half party, draw a card."
)]
#[case::the_party_die(
    "At the beginning of combat on your turn, roll the party die and create a number of Treasure tokens equal to the result."
)]
#[case::a_party_spell("Whenever you cast a party spell, draw a card.")]
fn near_misses_of_the_party_stay_with_the_table(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[rstest]
#[case::protection(
    "This creature has protection from the chosen player.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":{"Protection":[{"ControlledBy":"Chosen"}]}}]}}"#
)]
fn protection_from_the_chosen_player_is_a_static_grant(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::as_a_spell_effect(
    "Target creature gains protection from the chosen player until end of turn."
)]
#[case::from_a_player_by_name("This creature has protection from target player.")]
fn protection_from_the_chosen_player_needs_a_static_ability(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Spell, false), None, "{text}");
}
