//! Goal: "tap target creature", "untap target land" and the mills of a player ("target player mills three cards", "mill two cards") read in the shapes of the hand-written files, alone, in a trigger and after "you may"; the other things that turn permanents over or fill a graveyard stay with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::tap_a_creature(
    "Tap target creature.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Tap":{"what":{"Target":0}}}]}}"#
)]
#[case::untap_a_land(
    "Untap target land.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"Untap":{"what":{"Target":0}}}]}}"#
)]
#[case::untap_a_permanent(
    "Untap target permanent.",
    r#"{"Spell":{"targets":[{"what":{"Object":[]}}],"effects":[{"Untap":{"what":{"Target":0}}}]}}"#
)]
#[case::tap_what_an_opponent_controls(
    "Tap target creature an opponent controls.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Tap":{"what":{"Target":0}}}]}}"#
)]
#[case::tap_every_creature(
    "Tap all creatures.",
    r#"{"Spell":{"effects":[{"Tap":{"what":{"Each":[{"Type":"Creature"}]}}}]}}"#
)]
#[case::tap_every_creature_of_the_opponents(
    "Tap all creatures your opponents control.",
    r#"{"Spell":{"effects":[{"Tap":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}}]}}"#
)]
#[case::untap_every_land_you_control(
    "Untap all lands you control.",
    r#"{"Spell":{"effects":[{"Untap":{"what":{"Each":[{"Type":"Land"},{"ControlledBy":"You"}]}}}]}}"#
)]
#[case::tap_the_card("Tap ~.", r#"{"Spell":{"effects":[{"Tap":{"what":"This"}}]}}"#)]
#[case::untap_the_card("Untap ~.", r#"{"Spell":{"effects":[{"Untap":{"what":"This"}}]}}"#)]
#[case::may_untap(
    "You may untap target land.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"May":{"effects":[{"Untap":{"what":{"Target":0}}}]}}]}}"#
)]
#[case::tap_and_draw(
    "Tap target creature. Draw a card.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Tap":{"what":{"Target":0}}},{"Draw":{}}]}}"#
)]
#[case::untap_then_its_controller_loses_life(
    "Untap target creature. Its controller loses 1 life.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Untap":{"what":{"Target":0}}},{"LoseLife":{"who":{"ControllerOf":{"Target":0}},"amount":1}}]}}"#
)]
#[case::a_player_mills_a_card(
    "Target player mills a card.",
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Mill":{"who":{"Target":0}}}]}}"#
)]
#[case::a_player_mills_three(
    "Target player mills three cards.",
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Mill":{"who":{"Target":0},"count":3}}]}}"#
)]
#[case::an_opponent_mills_four(
    "Target opponent mills four cards.",
    r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"Mill":{"who":{"Target":0},"count":4}}]}}"#
)]
#[case::each_opponent_mills(
    "Each opponent mills two cards.",
    r#"{"Spell":{"effects":[{"Mill":{"who":"Opponents","count":2}}]}}"#
)]
#[case::each_player_mills(
    "Each player mills three cards.",
    r#"{"Spell":{"effects":[{"Mill":{"who":"EachPlayer","count":3}}]}}"#
)]
#[case::mill_two("Mill two cards.", r#"{"Spell":{"effects":[{"Mill":{"count":2}}]}}"#)]
#[case::mill_a_card("Mill a card.", r#"{"Spell":{"effects":[{"Mill":{}}]}}"#)]
#[case::mill_five("Mill five cards.", r#"{"Spell":{"effects":[{"Mill":{"count":5}}]}}"#)]
#[case::mill_then_draw(
    "Mill two cards, then draw a card.",
    r#"{"Spell":{"effects":[{"Mill":{"count":2}},{"Draw":{}}]}}"#
)]
fn the_effects_read_in_the_shape_of_the_hand_written_files(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::tap_what_attacks(
    "When ~ enters, tap target creature an opponent controls.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Tap":{"what":{"Target":0}}}]}}"#
)]
#[case::untap_when_attacking(
    "Whenever ~ attacks, untap ~.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Untap":{"what":"This"}}]}}"#
)]
#[case::untap_your_lands(
    "At the beginning of each end step, untap all lands you control.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Untap":{"what":{"Each":[{"Type":"Land"},{"ControlledBy":"You"}]}}}]}}"#
)]
#[case::an_opponent_mills_when_it_enters(
    "When ~ enters, target player mills three cards.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Player"}],"effects":[{"Mill":{"who":{"Target":0},"count":3}}]}}"#
)]
#[case::mill_at_your_upkeep(
    "At the beginning of your upkeep, mill a card.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Mill":{}}]}}"#
)]
fn the_effects_read_in_a_trigger(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case("Tap target creature.")]
#[case("Tap target artifact.")]
#[case("Tap target permanent.")]
#[case("Tap target land.")]
#[case("Tap target nonland permanent.")]
#[case("Tap target creature you control.")]
#[case("Tap target creature an opponent controls.")]
#[case("Tap target creature with flying.")]
#[case("Tap target nonblack creature.")]
#[case("Tap target Goblin.")]
#[case("Tap all creatures.")]
#[case("Tap all creatures you control.")]
#[case("Tap all creatures your opponents control.")]
#[case("Tap all lands.")]
#[case("Tap all artifacts.")]
#[case("Tap ~.")]
#[case("Tap this creature.")]
#[case("Untap target creature.")]
#[case("Untap target land.")]
#[case("Untap target permanent.")]
#[case("Untap target artifact.")]
#[case("Untap target creature you control.")]
#[case("Untap all creatures you control.")]
#[case("Untap all lands you control.")]
#[case("Untap all permanents you control.")]
#[case("Untap ~.")]
#[case("You may tap target creature.")]
#[case("You may untap target land.")]
#[case("You may untap ~.")]
#[case("Draw a card. Tap target creature.")]
#[case("Tap target creature. Draw a card.")]
#[case("Tap target creature. You gain 2 life.")]
#[case("Mill a card.")]
#[case("Mill two cards.")]
#[case("Mill three cards.")]
#[case("Mill five cards.")]
#[case("Target player mills a card.")]
#[case("Target player mills two cards.")]
#[case("Target player mills three cards.")]
#[case("Target player mills ten cards.")]
#[case("Target opponent mills four cards.")]
#[case("Each opponent mills two cards.")]
#[case("Each player mills three cards.")]
#[case("Mill two cards, then draw a card.")]
#[case("Draw a card, then mill two cards.")]
#[case("Mill three cards. Draw a card.")]
#[case("Draw a card. Mill a card.")]
#[case("Target player mills three cards. You gain 3 life.")]
#[case("Target opponent mills two cards and loses 2 life.")]
#[case("You may mill three cards.")]
fn more_oracle_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case("When ~ enters, tap target creature.")]
#[case("When ~ enters, tap target creature an opponent controls.")]
#[case("When ~ enters, you may tap target creature.")]
#[case("When ~ enters, tap all creatures your opponents control.")]
#[case("Whenever ~ attacks, untap ~.")]
#[case("Whenever ~ attacks, tap target creature.")]
#[case("At the beginning of each end step, untap all lands you control.")]
#[case("At the beginning of your upkeep, untap target land.")]
#[case("When ~ enters, target player mills three cards.")]
#[case("When ~ enters, each opponent mills two cards.")]
#[case("When ~ dies, target player mills three cards.")]
#[case("Whenever ~ attacks, each opponent mills a card.")]
#[case("At the beginning of your upkeep, mill a card.")]
#[case("At the beginning of your upkeep, you may mill two cards.")]
#[case("Whenever you cast a spell, mill a card.")]
fn more_oracle_wordings_in_a_trigger_read_and_write_back(#[case] text: &str) {
    assert!(trigger(text).is_some(), "{text}");
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::either("Tap or untap target creature.")]
#[case::tap_or_untap_a_permanent("Tap or untap target permanent.")]
#[case::it_stays_tapped(
    "Tap target creature. It doesn't untap during its controller's next untap step."
)]
#[case::a_gift("Untap target creature you control. It gains vigilance until end of turn.")]
#[case::a_pronoun("Untap it.")]
#[case::a_pronoun_to_tap("Tap it.")]
#[case::enchanted("Tap enchanted creature.")]
#[case::equipped("Untap equipped creature.")]
#[case::a_spell("Tap target spell.")]
#[case::a_card_in_a_graveyard("Tap target creature card in a graveyard.")]
#[case::a_player("Tap target player.")]
#[case::the_players_lands("Untap all lands target player controls.")]
#[case::a_count("Untap X target lands.")]
#[case::for_each("Tap target creature for each card in your hand.")]
#[case::a_condition("If you control a Goblin, tap target creature.")]
#[case::in_a_cost("Tap two untapped creatures you control.")]
#[case::in_an_ability_cost("Tap an untapped creature you control.")]
#[case::no_object("Tap.")]
#[case::untap_nothing("Untap.")]
#[case::a_duration("Tap target creature until end of turn.")]
#[case::unless("Tap target creature unless its controller pays {2}.")]
#[case::three_targets("Tap target creature, target land, and target artifact.")]
#[case::another_word_order("You mill two cards.")]
#[case::a_mana_sentence("Untap target land. Add {G}.")]
#[case::x_cards("Mill X cards.")]
#[case::as_many_as_the_life("Target player mills cards equal to their life total.")]
#[case::the_top_of_the_library("Target player mills the top three cards of their library.")]
#[case::the_long_form("Put the top three cards of your library into your graveyard.")]
#[case::then_return("Mill three cards, then return a card from your graveyard to your hand.")]
#[case::no_number("Mill cards.")]
#[case::a_number_without_cards("Mill three.")]
#[case::a_creature("Target creature mills three cards.")]
#[case::for_each_card("Each opponent mills a card for each creature you control.")]
#[case::half("Target player mills half their library.")]
#[case::until("Target player mills cards until a land card is milled.")]
#[case::all_players_but_you("Each other player mills three cards.")]
fn the_things_the_templates_do_not_model_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[test]
fn a_tap_and_an_untap_of_the_same_permanents_are_different_abilities() {
    let tap = spell("Tap target creature.");
    let untap = spell("Untap target creature.");
    assert!(tap.is_some() && untap.is_some());
    assert_ne!(tap, untap);
}

#[test]
fn a_mill_names_who_mills_and_how_many() {
    let you = spell("Mill two cards.");
    let them = spell("Each opponent mills two cards.");
    let more = spell("Mill three cards.");
    assert!(you.is_some() && them.is_some() && more.is_some());
    assert_ne!(you, them);
    assert_ne!(you, more);
}
