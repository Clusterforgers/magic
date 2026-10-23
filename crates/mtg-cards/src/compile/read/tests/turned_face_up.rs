//! Goal: "When ~ is turned face up, ..." reads as the `TurnedFaceUp` pattern with the effects after it, written back as the
//! same text ("when" for the card itself, "whenever" for any other permanent), and a wording the template does not model stays
//! with the table.
use super::*;

#[rstest]
#[case::destroy_an_artifact_or_an_enchantment(
    "When ~ is turned face up, destroy target artifact or enchantment.",
    r#"{"Triggered":{"when":{"TurnedFaceUp":{"what":["This"]}},"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#
)]
#[case::destroy_a_big_creature_of_an_opponent(
    "When ~ is turned face up, destroy target creature with power 4 or greater an opponent controls.",
    r#"{"Triggered":{"when":{"TurnedFaceUp":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":4}},{"ControlledBy":"Opponents"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#
)]
#[case::a_pump_for_the_team(
    "When ~ is turned face up, creatures you control get +2/+2 until end of turn.",
    r#"{"Triggered":{"when":{"TurnedFaceUp":{"what":["This"]}},"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":2,"toughness":2}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::tokens(
    "When ~ is turned face up, create four 1/1 green Saproling creature tokens.",
    r#"{"Triggered":{"when":{"TurnedFaceUp":{"what":["This"]}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Saproling","colors":["Green"],"types":["Creature"],"subtypes":["Saproling"],"power":1,"toughness":1}},"count":4}}]}}"#
)]
#[case::a_counter(
    "When ~ is turned face up, counter target instant or sorcery spell.",
    r#"{"Triggered":{"when":{"TurnedFaceUp":{"what":["This"]}},"targets":[{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#
)]
#[case::another_permanent(
    "Whenever another permanent you control is turned face up, draw a card.",
    r#"{"Triggered":{"when":{"TurnedFaceUp":{"what":["Other",{"ControlledBy":"You"}]}},"effects":[{"Draw":{}}]}}"#
)]
fn being_turned_face_up_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::whenever_for_the_card_itself("Whenever ~ is turned face up, draw a card.")]
#[case::a_condition_on_the_permanent(
    "Whenever another permanent you control is turned face up, if it's a creature, put two +1/+1 counters on it."
)]
#[case::the_face_it_returns_in(
    "Whenever a permanent you control is turned face up, you may return this card from your graveyard to the battlefield face up or face down."
)]
fn a_wording_the_template_does_not_model_is_left_to_the_table(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Triggered, false).is_none(),
        "{text}"
    );
}
