//! Goal: a triggered clause is an event, a comma and sentences the effect templates read; the events the DSL has patterns for read into it and write back as the card's own text, and the ones it has none for, and any condition it does not model, stay with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

fn reads(text: &str, json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::enters(
    "When ~ enters, draw two cards.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{"count":2}}]}}"#
)]
#[case::this_creature(
    "When this creature enters, draw a card.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::dies(
    "When ~ dies, you lose 2 life.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"LoseLife":{"amount":2}}]}}"#
)]
#[case::leaves(
    "When ~ leaves the battlefield, each opponent loses 2 life.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield"}},"effects":[{"LoseLife":{"who":"Opponents","amount":2}}]}}"#
)]
#[case::attacks(
    "Whenever ~ attacks, scry 1.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Scry":{}}]}}"#
)]
#[case::this_artifact(
    "When this artifact enters, you gain 3 life.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"GainLife":{"amount":3}}]}}"#
)]
#[case::enters_or_attacks(
    "Whenever ~ enters or attacks, draw a card.",
    r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"Attacks":{"what":["This"]}}]},"effects":[{"Draw":{}}]}}"#
)]
#[case::enters_or_dies(
    "When ~ enters or dies, draw a card.",
    r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}}]},"effects":[{"Draw":{}}]}}"#
)]
#[case::cast_a_type(
    "Whenever you cast a creature spell, draw a card.",
    r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Type":"Creature"}],"by":"You"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::cast_any(
    "Whenever you cast a spell, you gain 1 life.",
    r#"{"Triggered":{"when":{"SpellCast":{"by":"You"}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::opponent_casts(
    "Whenever an opponent casts a spell, you gain 1 life.",
    r#"{"Triggered":{"when":{"SpellCast":{"by":"Opponents"}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::a_player_casts(
    "Whenever a player casts a spell, each opponent loses 1 life.",
    r#"{"Triggered":{"when":{"SpellCast":{}},"effects":[{"LoseLife":{"who":"Opponents","amount":1}}]}}"#
)]
#[case::noncreature(
    "Whenever you cast a noncreature spell, ~ deals 1 damage to each opponent.",
    r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Not":{"Type":"Creature"}}],"by":"You"}},"effects":[{"DealDamage":{"to":"Opponents","amount":1}}]}}"#
)]
#[case::instant_or_sorcery(
    "Whenever you cast an instant or sorcery spell, scry 1.",
    r#"{"Triggered":{"when":{"SpellCast":{"what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}],"by":"You"}},"effects":[{"Scry":{}}]}}"#
)]
#[case::sacrifice(
    "Whenever you sacrifice a creature, draw a card.",
    r#"{"Triggered":{"when":{"Sacrificed":{"what":[{"Type":"Creature"}],"by":"You"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::upkeep(
    "At the beginning of your upkeep, draw a card.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::each_upkeep(
    "At the beginning of each upkeep, you lose 1 life.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep"}},"effects":[{"LoseLife":{"amount":1}}]}}"#
)]
#[case::end_step(
    "At the beginning of your end step, scry 1.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"Scry":{}}]}}"#
)]
#[case::each_end_step(
    "At the beginning of each end step, each player draws a card.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Draw":{"who":"EachPlayer"}}]}}"#
)]
#[case::draw_step(
    "At the beginning of your draw step, draw a card.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Draw","whose":"You"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::combat_on_your_turn(
    "At the beginning of combat on your turn, scry 1.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"effects":[{"Scry":{}}]}}"#
)]
#[case::a_creature_you_control_enters(
    "Whenever a creature you control enters, you gain 1 life.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"Battlefield"}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::another_creature_dies(
    "Whenever another creature you control dies, draw a card.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::an_opponents_creature_dies(
    "Whenever a creature an opponent controls dies, you gain 1 life.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"Opponents"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::a_creature_attacks(
    "Whenever a creature you control attacks, scry 1.",
    r#"{"Triggered":{"when":{"Attacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"effects":[{"Scry":{}}]}}"#
)]
#[case::an_intervening_if(
    "At the beginning of your upkeep, if you have 40 or more life, draw a card.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":40}},"effects":[{"Draw":{}}]}}"#
)]
#[case::an_intervening_if_you_are_the_monarch(
    "At the beginning of your upkeep, if you're the monarch, draw a card.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Monarch":"You"},"effects":[{"Draw":{}}]}}"#
)]
#[case::an_intervening_if_an_opponent_is_the_monarch(
    "At the beginning of your end step, if an opponent is the monarch, you gain 2 life.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"if":{"Monarch":"Opponents"},"effects":[{"GainLife":{"amount":2}}]}}"#
)]
#[case::two_sentences(
    "When ~ enters, draw a card. You gain 2 life.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{}},{"GainLife":{"amount":2}}]}}"#
)]
#[case::a_target(
    "When ~ enters, destroy target creature an opponent controls.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#
)]
fn the_events_read_into_the_dsl_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads(text, json);
}

#[rstest]
#[case("When ~ enters, scry 2.")]
#[case("When ~ enters, you gain 3 life.")]
#[case("When ~ enters, ~ deals 2 damage to any target.")]
#[case("When ~ enters, exile target creature an opponent controls.")]
#[case("When ~ enters, destroy target nonland permanent.")]
#[case("When ~ dies, each opponent loses 2 life.")]
#[case("When ~ dies, draw a card.")]
#[case("When ~ leaves the battlefield, draw a card.")]
#[case("When ~ enters or leaves the battlefield, draw a card.")]
#[case("Whenever ~ attacks, target creature gets +2/+0 until end of turn.")]
#[case("Whenever ~ attacks, each opponent loses 1 life.")]
#[case("Whenever ~ attacks, ~ gets +1/+1 until end of turn.")]
#[case("Whenever ~ enters or attacks, scry 2.")]
#[case("Whenever you cast a noncreature spell, scry 1.")]
#[case("Whenever you cast a creature spell, you gain 1 life.")]
#[case("Whenever you cast an artifact spell, draw a card.")]
#[case("Whenever you cast an enchantment spell, draw a card.")]
#[case("Whenever you cast a red spell, ~ deals 1 damage to each opponent.")]
#[case("Whenever an opponent casts a creature spell, you gain 1 life.")]
#[case("Whenever an opponent casts a noncreature spell, you gain 1 life.")]
#[case("Whenever a player casts a spell, scry 1.")]
#[case("Whenever you sacrifice another permanent, draw a card.")]
#[case("Whenever you sacrifice an artifact, draw a card.")]
#[case("Whenever an opponent sacrifices a creature, you gain 1 life.")]
#[case("At the beginning of your upkeep, you gain 1 life.")]
#[case("At the beginning of your upkeep, scry 1.")]
#[case("At the beginning of each opponent's upkeep, each opponent loses 1 life.")]
#[case("At the beginning of each upkeep, each player draws a card.")]
#[case("At the beginning of your end step, draw a card.")]
#[case("At the beginning of each end step, you gain 1 life.")]
#[case("At the beginning of each opponent's end step, each opponent discards a card.")]
#[case("At the beginning of your draw step, scry 1.")]
#[case("At the beginning of your precombat main phase, scry 1.")]
#[case("At the beginning of your postcombat main phase, draw a card.")]
#[case("At the beginning of each combat, you gain 1 life.")]
#[case("At the beginning of combat on your turn, target creature gets +1/+1 until end of turn.")]
#[case("Whenever a creature enters, you gain 1 life.")]
#[case("Whenever another creature enters, you gain 1 life.")]
#[case("Whenever a creature you control enters, scry 1.")]
#[case("Whenever a nontoken creature you control dies, draw a card.")]
#[case("Whenever a creature an opponent controls enters, you gain 1 life.")]
#[case("Whenever an artifact or creature you control dies, draw a card.")]
#[case("Whenever another creature dies, you gain 1 life.")]
#[case("Whenever a land you control enters, you gain 1 life.")]
#[case("Whenever a nonland permanent you control dies, scry 1.")]
#[case("Whenever a creature you control attacks, draw a card.")]
#[case("Whenever another creature you control leaves the battlefield, scry 1.")]
#[case("Whenever an artifact enters, draw a card.")]
#[case("Whenever a Clue you control enters, scry 1.")]
#[case("Whenever an Elf you control dies, scry 1.")]
#[case("Whenever you sacrifice a Treasure, scry 1.")]
fn more_oracle_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::landfall(
    "Landfall \u{2014} Whenever a land you control enters, draw a card.",
    "Whenever a land you control enters, draw a card."
)]
#[case::a_longer_word(
    "Spell mastery \u{2014} When ~ enters, scry 2.",
    "When ~ enters, scry 2."
)]
#[case::constellation(
    "Constellation \u{2014} Whenever an enchantment you control enters, you gain 1 life.",
    "Whenever an enchantment you control enters, you gain 1 life."
)]
fn an_ability_word_is_flavor_and_the_rest_is_read(#[case] text: &str, #[case] rest: &str) {
    let read = trigger(text).unwrap_or_else(|| panic!("not read: {text}"));
    assert_eq!(read, golden(rest, ClauseKind::Triggered, false), "{text}");
}

#[rstest]
#[case::a_name_is_not_a_subtype("Whenever another Commander dies, draw a card.")]
#[case::a_plural_subtype("Whenever Elves you control enter, draw a card.")]
#[case::a_card_name("Whenever you sacrifice a Sol Ring, draw a card.")]
#[case::delayed("At the beginning of the next end step, draw a card.")]
#[case::delayed_upkeep("At the beginning of the next upkeep, draw a card.")]
#[case::a_step_alone("At the beginning of draw a card.")]
#[case::an_unmodelled_step("At the beginning of your declare blockers step, draw a card.")]
#[case::combat_without_a_turn("At the beginning of combat, scry 1.")]
#[case::opponents_combat("At the beginning of combat on each opponent's turn, scry 1.")]
#[case::end_of_combat("At end of combat, draw a card.")]
#[case::gain_life_of_an_amount("Whenever you gain 3 life, draw a card.")]
#[case::lose_life_of_a_creature("Whenever a creature loses life, you gain 1 life.")]
#[case::discard_a_land("Whenever you discard a land card, draw a card.")]
#[case::plural_subject("Whenever Elves you control enter, draw a card.")]
#[case::dies_or_exiled("Whenever a creature you control dies or is put into exile, scry 1.")]
#[case::cast_a_named_spell("Whenever you cast a spell named Bolt, draw a card.")]
#[case::cast_another("Whenever you cast another spell, draw a card.")]
#[case::enters_under_your_control(
    "Whenever a creature enters the battlefield under your control, scry 1."
)]
#[case::when_for_a_repeating_event("When ~ attacks, draw a card.")]
#[case::whenever_for_a_single_event("Whenever ~ enters, draw a card.")]
#[case::when_for_another_subject("When a creature you control dies, draw a card.")]
#[case::no_article("Whenever creature you control dies, draw a card.")]
#[case::a_wrong_article("Whenever an creature you control dies, draw a card.")]
#[case::another_with_an_article("Whenever an another creature dies, draw a card.")]
#[case::a_repeated_verb("When ~ enters or enters, draw a card.")]
#[case::a_condition(
    "At the beginning of your upkeep, if an opponent controls more lands than you, draw a card."
)]
#[case::a_cast_condition("When ~ enters, if you cast it, draw a card.")]
#[case::a_life_total_for_an_opponent(
    "At the beginning of your upkeep, if an opponent has 10 or less life, draw a card."
)]
#[case::life_exactly("At the beginning of your upkeep, if you have 20 life, draw a card.")]
#[case::an_if_without_a_comma(
    "At the beginning of your upkeep, if you have 40 or more life draw a card."
)]
#[case::two_modes_of_three(
    "Whenever ~ attacks, choose two \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.\n\u{2022} Scry 1."
)]
#[case::its_power("When ~ dies, you gain life equal to its power.")]
#[case::it_is_another_object("Whenever a creature enters, it deals 2 damage to any target.")]
#[case::an_unread_effect("When ~ enters, flip a coin.")]
#[case::a_later_unread_sentence("When ~ enters, draw a card. Flip a coin.")]
#[case::an_if_you_do("When ~ enters, draw a card. If you do, scry 1.")]
#[case::no_comma("When ~ enters draw a card.")]
#[case::no_effect("When ~ enters,")]
#[case::no_effect_at_all("When ~ enters.")]
#[case::only_the_event("Whenever ~ attacks")]
#[case::not_a_trigger("Draw a card.")]
#[case::lowercase_start("whenever ~ attacks, draw a card.")]
#[case::empty("")]
fn near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text:?}");
}

#[test]
fn the_targets_of_a_trigger_are_those_its_sentences_ask_for() {
    let Some(Ability::Triggered {
        targets, effects, ..
    }) = trigger("When ~ enters, destroy target creature. Its controller gains 3 life.")
    else {
        panic!("not read");
    };
    assert_eq!(targets.len(), 1);
    assert_eq!(effects.len(), 2);
    assert_eq!(
        trigger("When ~ enters, destroy target creature. Exile target land."),
        None
    );
}

#[test]
fn a_chapter_or_a_spell_line_is_not_a_trigger() {
    assert_eq!(trigger("I \u{2014} Draw a card."), None);
    assert_eq!(spell("When ~ enters, draw a card."), None);
}
