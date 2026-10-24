//! Goal: the sentence that keeps mana in the pool ("Until end of turn, you don't lose this mana as steps and phases end.", also "Until end of combat, ... as steps end.") reads after the sentence that adds the mana, after the sentence that restricts it and after the life gained with it, "that much" mana reads as the amount of the event, each writes back as the card's text, and a near-miss stays with the table.
use super::*;

#[rstest]
#[case::six_mana(
    "Whenever this creature attacks, add {R}{R}{R}{G}{G}{G}. Until end of turn, you don't lose this mana as steps and phases end.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"AddMana":{"mana":{"Fixed":["Red","Red","Red","Green","Green","Green"]},"lasts":"Turn"}}]}}"#
)]
#[case::until_end_of_combat(
    "Whenever this creature attacks, add {R}{R}. Until end of combat, you don't lose this mana as steps end.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"AddMana":{"mana":{"Fixed":["Red","Red"]},"lasts":"Combat"}}]}}"#
)]
#[case::with_life_gained(
    "Whenever this creature attacks, add {W}{W}{W} and you gain 3 life. Until end of turn, you don't lose this mana as steps and phases end.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"AddMana":{"mana":{"Fixed":["White","White","White"]},"lasts":"Turn"}},{"GainLife":{"amount":3}}]}}"#
)]
#[case::counted_and_restricted(
    "Whenever this creature attacks, add X mana in any combination of colors, where X is the total power of attacking creatures. Spend this mana only to cast spells. Until end of turn, you don't lose this mana as steps and phases end.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":{"Total":{"of":[{"Type":"Creature"},"Attacking"],"stat":"Power"}}}},"only":[{"Spell":{}}],"lasts":"Turn"}}]}}"#
)]
#[case::that_much_of_one_color(
    "Whenever this creature deals combat damage to a player, add that much mana of any one color. Until end of turn, you don't lose this mana as steps and phases end.",
    r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"AddMana":{"mana":{"AnyColor":"Triggering"},"lasts":"Turn"}}]}}"#
)]
#[case::that_much_of_a_type(
    "Whenever this creature deals combat damage to a player, add that much {G}. Until end of turn, you don't lose this mana as steps and phases end.",
    r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"times":"Triggering","lasts":"Turn"}}]}}"#
)]
fn mana_that_stays_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::the_mana_is_lost(
    "Whenever this creature attacks, add {R}. Until end of turn, you lose this mana as steps and phases end."
)]
#[case::the_wrong_length(
    "Whenever this creature attacks, add {R}. Until end of the game, you don't lose this mana as steps and phases end."
)]
#[case::a_turn_with_the_steps_of_a_phase(
    "Whenever this creature attacks, add {R}. Until end of turn, you don't lose this mana as steps end."
)]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Triggered, false).is_none(),
        "{text}"
    );
}
