//! Goal: "exile ~", "return target creature to its owner's hand" and "all creatures get -N/-N until end of turn" read in the shapes of the hand-written files, alone, in a trigger and after "you may"; the other things a card returns, exiles or shrinks stay with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::exile_the_card("Exile ~.", r#"{"Spell":{"effects":[{"Exile":{"what":"This"}}]}}"#)]
#[case::exile_this_creature(
    "Exile this creature.",
    r#"{"Spell":{"effects":[{"Exile":{"what":"This"}}]}}"#
)]
#[case::may_exile_the_card(
    "You may exile ~.",
    r#"{"Spell":{"effects":[{"May":{"effects":[{"Exile":{"what":"This"}}]}}]}}"#
)]
#[case::bounce(
    "Return target creature to its owner's hand.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#
)]
#[case::bounce_a_nonland_permanent(
    "Return target nonland permanent to its owner's hand.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Not":{"Type":"Land"}}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#
)]
#[case::bounce_and_draw(
    "Return target creature to its owner's hand. Draw a card.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}},{"Draw":{}}]}}"#
)]
#[case::bounce_its_controller_loses(
    "Return target creature to its owner's hand. Its controller loses 1 life.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}},{"LoseLife":{"who":{"ControllerOf":{"Target":0}},"amount":1}}]}}"#
)]
#[case::shrink_all(
    "All creatures get -1/-1 until end of turn.",
    r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"ModifyPT":{"power":-1,"toughness":-1}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::grow_all(
    "All creatures get +2/+2 until end of turn.",
    r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"ModifyPT":{"power":2,"toughness":2}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::shrink_yours(
    "Creatures you control get -1/-1 until end of turn.",
    r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":-1,"toughness":-1}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::grow_with_a_keyword(
    "All creatures get +1/+1 and gain flying until end of turn.",
    r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Flying"}],"duration":"UntilEndOfTurn"}}]}}"#
)]
fn the_effects_read_in_the_shape_of_the_hand_written_files(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::exile_itself(
    "Whenever ~ attacks, exile ~.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Exile":{"what":"This"}}]}}"#
)]
#[case::man_o_war(
    "When ~ enters, return target creature to its owner's hand.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#
)]
#[case::massacre_wurm(
    "When ~ enters, creatures your opponents control get -2/-2 until end of turn.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"mods":[{"ModifyPT":{"power":-2,"toughness":-2}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
fn the_effects_read_in_a_trigger(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case("Exile ~.")]
#[case("Exile this creature.")]
#[case("Exile this artifact.")]
#[case("Exile this enchantment.")]
#[case("You may exile ~.")]
#[case("Draw a card. Exile ~.")]
#[case("Exile ~. You gain 2 life.")]
#[case("Return target creature to its owner's hand.")]
#[case("Return target permanent to its owner's hand.")]
#[case("Return target nonland permanent to its owner's hand.")]
#[case("Return target artifact to its owner's hand.")]
#[case("Return target enchantment to its owner's hand.")]
#[case("Return target land to its owner's hand.")]
#[case("Return target creature you control to its owner's hand.")]
#[case("Return target creature an opponent controls to its owner's hand.")]
#[case("Return target nonblack creature to its owner's hand.")]
#[case("Return target tapped creature to its owner's hand.")]
#[case("Return target creature to its owner's hand. Draw a card.")]
#[case("Draw a card. Return target creature to its owner's hand.")]
#[case("You may return target creature to its owner's hand.")]
#[case("Return target Goblin to its owner's hand.")]
#[case("Return target creature to its owner's hand. Its controller loses 1 life.")]
#[case("Return target creature to its owner's hand. Its controller draws a card.")]
#[case("All creatures get -1/-1 until end of turn.")]
#[case("All creatures get -2/-2 until end of turn.")]
#[case("All creatures get -4/-4 until end of turn.")]
#[case("All creatures get +1/+1 until end of turn.")]
#[case("All creatures get +2/+0 until end of turn.")]
#[case("All creatures get +0/-1 until end of turn.")]
#[case("All creatures get +1/+1 and gain trample until end of turn.")]
#[case("All creatures gain flying until end of turn.")]
#[case("All lands get +1/+1 until end of turn.")]
#[case("All artifacts get -1/-1 until end of turn.")]
#[case("Creatures you control get -1/-1 until end of turn.")]
#[case("Creatures you control get +1/+1 until end of turn.")]
#[case("Creatures your opponents control get -1/-1 until end of turn.")]
#[case("Creatures your opponents control get -3/-3 until end of turn.")]
fn more_oracle_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case("When ~ enters, return target creature to its owner's hand.")]
#[case("When ~ enters, you may return target creature to its owner's hand.")]
#[case("When ~ enters, return target nonland permanent to its owner's hand.")]
#[case("When ~ enters, return target creature an opponent controls to its owner's hand.")]
#[case("Whenever ~ attacks, return target creature to its owner's hand.")]
#[case("Whenever ~ attacks, exile ~.")]
#[case("At the beginning of your end step, exile ~.")]
#[case("When ~ enters, all creatures get -1/-1 until end of turn.")]
#[case("When ~ dies, all creatures get -2/-2 until end of turn.")]
#[case("Whenever ~ attacks, creatures you control get +1/+1 until end of turn.")]
#[case("Whenever you cast a spell, all creatures get +1/+1 until end of turn.")]
fn more_oracle_wordings_in_a_trigger_read_and_write_back(#[case] text: &str) {
    assert!(trigger(text).is_some(), "{text}");
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::destroy_the_card("Destroy ~.")]
#[case::counter_the_card("Counter ~.")]
#[case::exile_and_a_target("Exile ~ and target creature.")]
#[case::exile_until("Exile ~ until end of turn.")]
#[case::exile_it("Exile it.")]
#[case::return_the_card("Return ~ to its owner's hand.")]
#[case::to_your_hand("Return target creature to your hand.")]
#[case::to_the_library("Return target creature to its owner's library.")]
#[case::to_the_battlefield("Return target creature to the battlefield.")]
#[case::from_a_graveyard("Return target card from a graveyard to its owner's hand.")]
#[case::from_your_graveyard_to_its_owners_hand(
    "Return target card from your graveyard to its owner's hand."
)]
#[case::a_spell("Return target spell to its owner's hand.")]
#[case::two_targets("Return up to two target creatures to their owners' hands.")]
#[case::all("Return all creatures to their owners' hands.")]
#[case::each("Return each creature to its owner's hand.")]
#[case::a_player("Return target player to its owner's hand.")]
#[case::a_second_target("Return target creature and target land to their owners' hands.")]
#[case::unless("Return target creature to its owner's hand unless its controller pays {2}.")]
#[case::a_pronoun("Return it to its owner's hand.")]
#[case::no_hand("Return target creature to its owner's")]
#[case::no_duration("All creatures get -1/-1.")]
#[case::until_your_next_turn("All creatures get -1/-1 until your next turn.")]
#[case::all_you_control("All creatures you control get +1/+1 until end of turn.")]
#[case::all_attacking("All attacking creatures get +1/+0 until end of turn.")]
#[case::without_all("Creatures get -1/-1 until end of turn.")]
#[case::x("All creatures get -X/-X until end of turn.")]
#[case::a_subtype("All Goblins get +1/+1 until end of turn.")]
#[case::a_conditional("If you control a Goblin, all creatures get -1/-1 until end of turn.")]
#[case::one_number("All creatures get -1 until end of turn.")]
fn the_things_the_templates_do_not_model_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[test]
fn bouncing_a_creature_names_its_owners_hand_and_a_return_from_the_graveyard_your_own() {
    let bounce = spell("Return target creature to its owner's hand.");
    let raise = spell("Return target creature card from your graveyard to your hand.");
    assert!(bounce.is_some() && raise.is_some());
    assert_ne!(bounce, raise);
}
