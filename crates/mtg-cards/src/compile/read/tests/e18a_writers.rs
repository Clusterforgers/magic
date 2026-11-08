//! Goal: pack e18a writes the first ability of a kind each turn as the cards print it ("you may pay {0} rather than pay the equip cost of the first equip ability you activate each turn", during each of your turns, the first card you cycle, cycling costs as long as a condition holds, equip abilities at instant speed) and the turn's activations by kind ("you cycled two or more cards this turn", a loyalty ability activated, a spell cast from or an ability activated in a graveyard); another kind, another number, a missing condition or a lost turn reads Differs.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn first(kind: &str, op: &str, right: u32) -> String {
    format!(
        r#"{{"Compare":{{"left":{{"Tally":{{"event":{{"KindActivated":"{kind}"}},"filter":[],"who":"You"}}}},"op":"{op}","right":{right}}}}}"#
    )
}

fn free(kind: &str, condition: Option<String>) -> String {
    let condition = condition.map_or_else(String::new, |held| format!(r#","condition":{held}"#));
    format!(r#"{{"FreeActivation":{{"kind":"{kind}"{condition}}}}}"#)
}

fn your_turns(held: &str) -> String {
    format!(r#"{{"All":["YourTurn",{held}]}}"#)
}

const BRUENOR: &str = "You may pay {0} rather than pay the equip cost of the first equip ability you activate each turn.";
const FORGE_ANEW: &str = "You may pay {0} rather than pay the equip cost of the first equip ability you activate during each of your turns.";
const GAVI: &str =
    "You may pay {0} rather than pay the cycling cost of the first card you cycle each turn.";
const NEW_PERSPECTIVES: &str = "As long as you have seven or more cards in hand, you may pay {0} rather than pay cycling costs.";
const SEVEN_IN_HAND: &str =
    r#"{"Compare":{"left":{"Count":[{"OwnedBy":"You"},{"In":"Hand"}]},"op":"Ge","right":7}}"#;

#[rstest]
#[case::bruenor(free("Equip", Some(first("Equip", "Lt", 1))), BRUENOR, true)]
#[case::none_yet(free("Equip", Some(first("Equip", "Eq", 0))), BRUENOR, true)]
#[case::forge_anew(
    free("Equip", Some(your_turns(&first("Equip", "Lt", 1)))),
    FORGE_ANEW,
    true
)]
#[case::gavi(free("Cycling", Some(first("Cycling", "Lt", 1))), GAVI, true)]
#[case::new_perspectives(free("Cycling", Some(SEVEN_IN_HAND.to_owned())), NEW_PERSPECTIVES, true)]
#[case::every_equip(
    free("Equip", None),
    "You may pay {0} rather than pay equip costs.",
    true
)]
#[case::another_kind(free("Cycling", Some(first("Cycling", "Lt", 1))), BRUENOR, false)]
#[case::the_tally_of_another_kind(free("Equip", Some(first("Cycling", "Lt", 1))), BRUENOR, false)]
#[case::the_second(free("Equip", Some(first("Equip", "Lt", 2))), BRUENOR, false)]
#[case::no_condition(free("Equip", None), BRUENOR, false)]
#[case::lost_turn(free("Equip", Some(first("Equip", "Lt", 1))), FORGE_ANEW, false)]
#[case::every_turn_for_yours(
    free("Equip", Some(your_turns(&first("Equip", "Lt", 1)))),
    BRUENOR,
    false
)]
#[case::without_the_hand(free("Cycling", None), NEW_PERSPECTIVES, false)]
#[case::shikari(
    r#"{"InstantActivation":{"kind":"Equip"}}"#.to_owned(),
    "You may activate equip abilities any time you could cast an instant.",
    true
)]
#[case::forge_anew_flash(
    r#"{"InstantActivation":{"kind":"Equip","condition":"YourTurn"}}"#.to_owned(),
    "During your turn, you may activate equip abilities any time you could cast an instant.",
    true
)]
#[case::flash_without_your_turn(
    r#"{"InstantActivation":{"kind":"Equip"}}"#.to_owned(),
    "During your turn, you may activate equip abilities any time you could cast an instant.",
    false
)]
#[case::flash_of_cycling(
    r#"{"InstantActivation":{"kind":"Cycling"}}"#.to_owned(),
    "You may activate equip abilities any time you could cast an instant.",
    false
)]
fn first_free_activations_read_back(
    #[case] json: String,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(written(&json, clause), same);
}

/// "{T}: You gain 3 life." with the condition `condition`.
fn gated(condition: &str) -> String {
    format!(
        r#"{{"Activated":{{"condition":{condition},"cost":["Tap"],"effects":[{{"GainLife":{{"amount":3}}}}]}}}}"#
    )
}

const GRAVEYARD: &str = r#"{"Compare":{"left":{"Sum":[{"Tally":{"event":"SpellCast","filter":[{"ControlledBy":"You"},{"WasCast":{"from":"Graveyard"}}],"who":"EachPlayer"}},{"Tally":{"event":"Activated","filter":[{"In":"Graveyard"}],"who":"You"}}]},"op":"Ge","right":1}}"#;
const CYCLED_TWO: &str =
    "{T}: You gain 3 life. Activate only if you cycled two or more cards this turn.";
const LOYALTY: &str =
    "{T}: You gain 3 life. Activate only if you've activated a loyalty ability this turn.";

#[rstest]
#[case::cycled_two(gated(&first("Cycling", "Ge", 2)), CYCLED_TWO, true)]
#[case::loyalty(gated(&first("Loyalty", "Ge", 1)), LOYALTY, true)]
#[case::graveyard(
    gated(GRAVEYARD),
    "{T}: You gain 3 life. Activate only if you've cast a spell from a graveyard or activated an ability of a card in a graveyard this turn.",
    true
)]
#[case::cycled_one_for_two(gated(&first("Cycling", "Ge", 1)), CYCLED_TWO, false)]
#[case::equip_for_cycling(gated(&first("Equip", "Ge", 2)), CYCLED_TWO, false)]
#[case::cycling_for_loyalty(gated(&first("Cycling", "Ge", 1)), LOYALTY, false)]
#[case::graveyard_twice(
    gated(&GRAVEYARD.replace(r#""right":1"#, r#""right":2"#)),
    "{T}: You gain 3 life. Activate only if you've cast a spell from a graveyard or activated an ability of a card in a graveyard this turn.",
    false
)]
fn activations_of_the_turn_read_back(
    #[case] json: String,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(written(&json, clause), same);
}
