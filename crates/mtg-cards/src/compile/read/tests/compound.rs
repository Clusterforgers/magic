//! Goal: one subject with several verb phrases ("target player draws two cards and loses 2 life", "target opponent sacrifices a creature, discards a card, and loses 3 life") reads as the effects of its sentence in order, a sentence of another subject follows, and any other subject, list, verb form or verb stays with the table.
use super::*;
use crate::effect::Effect;
use crate::refs::Ref;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::gain_and_draw(
    "You gain 3 life and draw a card.",
    false,
    r#"{"Spell":{"effects":[{"GainLife":{"amount":3}},{"Draw":{}}]}}"#
)]
#[case::sphinxs_revelation(
    "You gain X life and draw X cards.",
    true,
    r#"{"Spell":{"effects":[{"GainLife":{"amount":"X"}},{"Draw":{"count":"X"}}]}}"#
)]
#[case::sign_in_blood(
    "Target player draws two cards and loses 2 life.",
    false,
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Draw":{"who":{"Target":0},"count":2}},{"LoseLife":{"who":{"Target":0},"amount":2}}]}}"#
)]
#[case::each_opponent(
    "Each opponent discards a card and loses 2 life.",
    false,
    r#"{"Spell":{"effects":[{"Discard":{"who":"Opponents"}},{"LoseLife":{"who":"Opponents","amount":2}}]}}"#
)]
#[case::a_list_of_three(
    "Target player draws a card, discards a card, and loses 1 life.",
    false,
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Draw":{"who":{"Target":0}}},{"Discard":{"who":{"Target":0}}},{"LoseLife":{"who":{"Target":0},"amount":1}}]}}"#
)]
#[case::an_edict(
    "Target opponent sacrifices a creature.",
    false,
    r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"Sacrifice":{"who":{"Target":0},"filter":[{"Type":"Creature"}]}}]}}"#
)]
#[case::each_player_sacrifices(
    "Each player sacrifices a nonland permanent.",
    false,
    r#"{"Spell":{"effects":[{"Sacrifice":{"who":"EachPlayer","filter":[{"Not":{"Type":"Land"}}]}}]}}"#
)]
#[case::a_second_sentence(
    "Target opponent discards a card and loses 3 life. You draw a card and gain 3 life.",
    false,
    r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"Discard":{"who":{"Target":0}}},{"LoseLife":{"who":{"Target":0},"amount":3}},{"Draw":{}},{"GainLife":{"amount":3}}]}}"#
)]
fn compound_predicates_read_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] x_free: bool,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, x_free, json);
}

#[test]
fn archon_of_cruelty_reads_as_its_hand_written_file() {
    let text = "Whenever ~ enters or attacks, target opponent sacrifices a creature or planeswalker of their choice, discards a card, and loses 3 life. You draw a card and gain 3 life.";
    let json = r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"Attacks":{"what":["This"]}}]},"targets":[{"what":"Opponent"}],"effects":[{"Sacrifice":{"who":{"Target":0},"filter":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]}},{"Discard":{"who":{"Target":0}}},{"LoseLife":{"who":{"Target":0},"amount":3}},{"Draw":{}},{"GainLife":{"amount":3}}]}}"#;
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case("Target player draws a card and loses 1 life.")]
#[case("Target opponent discards two cards and loses 2 life.")]
#[case("Target opponent mills three cards and loses 3 life.")]
#[case("Each player draws a card and gains 1 life.")]
#[case("Each opponent loses 1 life and discards a card.")]
#[case("You mill two cards and draw a card.")]
#[case("You draw two cards and discard a card at random.")]
#[case("You gain 2 life and lose 1 life.")]
#[case("Target player discards a card, mills two cards, and draws a card.")]
#[case("Target player draws a card, gains 2 life, and loses 1 life.")]
#[case("Target opponent sacrifices a creature and loses 2 life.")]
#[case("Target opponent sacrifices an artifact, discards a card, and loses 2 life.")]
#[case("Target player sacrifices a creature.")]
#[case("Target opponent sacrifices a creature of their choice.")]
#[case("Each player sacrifices a creature.")]
#[case("Each opponent sacrifices a creature.")]
#[case("You sacrifice a creature.")]
#[case("Target opponent sacrifices a creature with flying.")]
#[case("Target opponent sacrifices an attacking creature.")]
#[case("Target opponent sacrifices a nontoken creature.")]
#[case("Target opponent sacrifices a Goblin.")]
#[case("Target opponent sacrifices an artifact or enchantment.")]
#[case("Target opponent loses 3 life. You gain 3 life.")]
#[case("Target opponent sacrifices a creature. You draw a card and gain 3 life.")]
#[case("Draw a card. Target opponent discards a card and loses 1 life.")]
#[case("Target player draws a card and loses 1 life. You gain 2 life.")]
fn more_oracle_wordings_of_one_subject_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case("When ~ enters, you gain 2 life and draw a card.")]
#[case("When ~ enters, target opponent discards a card and loses 2 life.")]
#[case("Whenever ~ attacks, each opponent loses 1 life and discards a card.")]
#[case("Whenever you cast a spell, you draw a card and lose 1 life.")]
#[case("At the beginning of your upkeep, each opponent sacrifices a creature of their choice.")]
#[case(
    "When ~ dies, target opponent sacrifices a creature or planeswalker, discards a card, and loses 3 life."
)]
fn more_oracle_wordings_of_one_subject_in_a_trigger_read_and_write_back(#[case] text: &str) {
    assert!(trigger(text).is_some(), "{text}");
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::a_second_subject("Target player draws two cards and each opponent loses 2 life.")]
#[case::a_repeated_target("Target player draws a card and target player loses 1 life.")]
#[case::then("Target player draws a card, then discards a card.")]
#[case::then_for_each(
    "Each player loses 1 life, discards a card, sacrifices a creature, then sacrifices a land."
)]
#[case::two_ands("Target player draws a card and loses 2 life and discards a card.")]
#[case::no_serial_comma("Target player draws a card, loses 2 life and discards a card.")]
#[case::a_comma_before_and("Target player draws a card, and loses 2 life.")]
#[case::an_unread_verb("Target player draws a card and taps a land.")]
#[case::an_unread_first_verb("Target player taps a land and draws a card.")]
#[case::an_unfinished_phrase("Target player draws a card and loses.")]
#[case::a_dangling_and("Target player draws a card and.")]
#[case::a_dangling_comma("Target player draws a card,.")]
#[case::for_each("Target player draws a card and loses 1 life for each creature you control.")]
#[case::wrong_verb_form("Target player draw a card and loses 2 life.")]
#[case::wrong_verb_form_for_you("You draws a card and gain 3 life.")]
#[case::wrong_second_verb_form("You draw a card and gains 3 life.")]
#[case::imperative("Draw a card and gain 3 life.")]
#[case::imperative_list("Draw a card, discard a card, and gain 3 life.")]
#[case::win("You gain 3 life and win the game.")]
#[case::unless("Target player draws a card and loses 1 life unless they discard a card.")]
#[case::if_you_do("Target player draws a card. If you do, they lose 1 life.")]
#[case::two_creatures("Target opponent sacrifices two creatures.")]
#[case::all_creatures("Target opponent sacrifices all creatures.")]
#[case::another("Target opponent sacrifices another creature.")]
#[case::a_creature_you_control("Target opponent sacrifices a creature you control.")]
#[case::in_a_zone("Target opponent sacrifices a creature card from your graveyard.")]
#[case::the_card_itself("Target opponent sacrifices ~.")]
#[case::imperative_sacrifice("Sacrifice a creature.")]
#[case::two_things("Target opponent sacrifices a creature and a planeswalker.")]
#[case::a_condition("Target opponent sacrifices a creature if you control a Goblin.")]
#[case::a_second_effect_of_a_chain("Target opponent sacrifices a creature, then discards a card.")]
#[case::a_player_that_is_not_targeted("That player draws a card and loses 1 life.")]
#[case::its_controller("Its controller draws a card and loses 1 life.")]
#[case::a_creature_subject("Target creature draws a card and loses 1 life.")]
#[case::any_target_subject("Any target draws a card and loses 1 life.")]
fn a_compound_the_template_does_not_model_stays_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[test]
fn the_verbs_of_one_sentence_share_its_subject_and_target() {
    let Some(Ability::Spell { targets, effects }) =
        spell("Target opponent sacrifices a creature, discards a card, and loses 3 life.")
    else {
        panic!("not read");
    };
    assert_eq!(targets.len(), 1);
    assert_eq!(effects.len(), 3);
    for effect in &effects {
        let who = match effect {
            Effect::Sacrifice { who, .. }
            | Effect::Discard { who, .. }
            | Effect::LoseLife { who, .. } => who,
            other => panic!("{other:?}"),
        };
        assert_eq!(*who, Ref::Target(0));
    }
}

#[rstest]
#[case::separate_sentences("Draw a card.\nYou gain 2 life.")]
#[case::joined("You draw a card and gain 2 life.")]
fn both_wordings_of_one_pair_of_effects_read_to_the_same_effects(#[case] text: &str) {
    let json = r#"{"Spell":{"effects":[{"Draw":{}},{"GainLife":{"amount":2}}]}}"#;
    reads_as(text, ClauseKind::Spell, false, json);
}

#[test]
fn a_compound_has_more_than_one_wording_and_a_second_target_cannot_be_named_twice() {
    let joined = ability(r#"{"Spell":{"effects":[{"Draw":{}},{"GainLife":{"amount":2}}]}}"#);
    let wordings = render_variants(&joined);
    assert_eq!(
        wordings,
        [
            "Draw a card. You gain 2 life.",
            "You draw a card and gain 2 life.",
            "You draw a card and you gain 2 life."
        ]
    );
    let targeted = ability(
        r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Draw":{"who":{"Target":0}}},{"LoseLife":{"who":{"Target":0},"amount":1}}]}}"#,
    );
    assert_eq!(
        render_variants(&targeted),
        ["Target player draws a card and loses 1 life."]
    );
}
