//! Goal: the wordings of the vocabulary pass (surveil, proliferate, time travel, becoming the monarch, a choice of mana, mana of the chosen color, paying energy, and the triggers of life, drawing and discarding) read into the DSL and write back as the card's own text, and the near-misses stay with the table.
use super::*;

#[rstest]
#[case::surveil(
    "Surveil 2.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Surveil":{"count":2}}]}}"#
)]
#[case::surveil_then_draw(
    "Surveil 1, then draw a card.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Surveil":{}},{"Draw":{}}]}}"#
)]
#[case::proliferate(
    "Proliferate.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Proliferate":{}}]}}"#
)]
#[case::proliferate_after_a_draw(
    "Draw a card, then proliferate.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Draw":{}},{"Proliferate":{}}]}}"#
)]
#[case::c8_time_travel(
    "Time travel.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"TimeTravel":{}}]}}"#
)]
#[case::c8_time_travel_then_draw(
    "Time travel, then draw a card.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"TimeTravel":{}},{"Draw":{}}]}}"#
)]
#[case::monarch_when_it_enters(
    "When ~ enters, you become the monarch.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"BecomeMonarch":{}}]}}"#
)]
#[case::surveil_at_upkeep(
    "At the beginning of your upkeep, surveil 1.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Surveil":{}}]}}"#
)]
#[case::surveil_for_mana_and_tap(
    "{1}, {T}: Surveil 1.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"Surveil":{}}]}}"#
)]
#[case::a_choice_of_two(
    "{T}: Add {W} or {U}.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OneOf":["White","Blue"]}}}]}}"#
)]
#[case::a_choice_of_three(
    "{T}: Add {B}, {R}, or {G}.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OneOf":["Black","Red","Green"]}}}]}}"#
)]
#[case::a_choice_with_colorless(
    "{T}: Add {C} or {G}.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OneOf":["Colorless","Green"]}}}]}}"#
)]
#[case::a_choice_of_lists(
    "{W/U}, {T}: Add {W}{W}, {W}{U}, or {U}{U}.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"Mana":"{W/U}"},"Tap"],"effects":[{"AddMana":{"mana":{"OneOfLists":[["White","White"],["White","Blue"],["Blue","Blue"]]}}}]}}"#
)]
#[case::a_choice_of_two_lists(
    "{T}: Add {C}{C} or {G}{G}.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OneOfLists":[["Colorless","Colorless"],["Green","Green"]]}}}]}}"#
)]
#[case::lists_of_different_lengths(
    "{T}: Add {R}, {R}{G}, or {G}{G}{G}.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OneOfLists":[["Red"],["Red","Green"],["Green","Green","Green"]]}}}]}}"#
)]
#[case::the_chosen_color(
    "{T}: Add one mana of the chosen color.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"ChosenColor":1}}}]}}"#
)]
#[case::paying_energy(
    "{T}, Pay {E}: Add one mana of any color.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":["Tap",{"PayEnergy":1}],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#
)]
#[case::paying_energy_twice(
    "{T}, Pay {E}{E}: Draw a card.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":["Tap",{"PayEnergy":2}],"effects":[{"Draw":{}}]}}"#
)]
#[case::you_gain_life(
    "Whenever you gain life, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"LifeGained":{"who":"You"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::an_opponent_loses_life(
    "Whenever an opponent loses life, you gain 1 life.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"LifeLost":{"who":"Opponents"}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::a_player_draws(
    "Whenever a player draws a card, scry 1.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Drew":{}},"effects":[{"Scry":{}}]}}"#
)]
#[case::you_discard(
    "Whenever you discard a card, you gain 1 life.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Discarded":{"by":"You"}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
fn the_wordings_read_into_the_dsl_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::surveil_nothing("Surveil 0.")]
#[case::another_player_surveils("Target player surveils 2.")]
#[case::each_player_surveils("Each player surveils 1.")]
#[case::proliferate_with_an_object("Proliferate each creature.")]
#[case::a_player_becomes_the_monarch_of_nothing("Target player becomes the monarch of the table.")]
#[case::an_alternative_wording("You become the Monarch of the realm.")]
#[case::three_alternatives_in_a_row("{T}: Add {W} or {U} or {B}.")]
#[case::one_alternative("{T}: Add {W} or.")]
#[case::the_chosen_color_twice("{T}: Add two mana of the chosen color.")]
#[case::energy_and_life("{T}, Pay {E} and 2 life: Draw a card.")]
#[case::no_energy("{T}, Pay: Draw a card.")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    for kind in [ClauseKind::Spell, ClauseKind::Activated] {
        assert_eq!(clause(text, kind, false), None, "{text}");
    }
}

#[rstest]
#[case::except_the_first(
    "Whenever an opponent draws a card except the first one they draw in each of their draw steps, you gain 1 life."
)]
#[case::an_amount_of_life("Whenever you gain 3 life, draw a card.")]
#[case::a_kind_of_card("Whenever you discard a land card, draw a card.")]
#[case::a_creature_loses_life("Whenever a creature loses life, draw a card.")]
fn triggers_the_pattern_does_not_model_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
