//! Goal: the Monarch wordings (becoming the monarch, no monarch, "as long as you're the monarch" before or after the static, "the monarch's" step and "the monarch controls", your commander as a source, an attack at an opponent who is the monarch, "as the turn began", the exile until an opponent becomes the monarch, the monarch held "while" an event happens and damage that causes no loss of life) read into the DSL, write back as the card's own text, and leave the near-misses to the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

fn stat(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

#[rstest]
#[case::you_become_it(
    "Whenever you become the monarch, draw a card.",
    r#"{"Triggered":{"when":{"BecomesMonarch":{"who":"You"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::an_opponent_becomes_it(
    "Whenever an opponent becomes the monarch, you gain 2 life.",
    r#"{"Triggered":{"when":{"BecomesMonarch":{"who":"Opponents"}},"effects":[{"GainLife":{"amount":2}}]}}"#
)]
#[case::a_player_becomes_it(
    "Whenever a player becomes the monarch, draw a card.",
    r#"{"Triggered":{"when":{"BecomesMonarch":{}},"effects":[{"Draw":{}}]}}"#
)]
#[case::that_player_is_the_new_monarch(
    "Whenever an opponent becomes the monarch, that player loses 2 life.",
    r#"{"Triggered":{"when":{"BecomesMonarch":{"who":"Opponents"}},"effects":[{"LoseLife":{"who":"Triggering","amount":2}}]}}"#
)]
fn becoming_the_monarch_is_an_event(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::no_monarch_on_entering(
    "When ~ enters, if there is no monarch, you become the monarch.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"Not":{"Monarch":"EachPlayer"}},"effects":[{"BecomeMonarch":{}}]}}"#
)]
#[case::no_monarch_in_the_end_step(
    "At the beginning of your end step, if there is no monarch, target opponent becomes the monarch.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"if":{"Not":{"Monarch":"EachPlayer"}},"targets":[{"what":"Opponent"}],"effects":[{"BecomeMonarch":{"who":{"Target":0}}}]}}"#
)]
#[case::an_opponent_is_the_monarch(
    "At the beginning of your upkeep, if an opponent is the monarch, draw a card.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Monarch":"Opponents"},"effects":[{"Draw":{}}]}}"#
)]
fn an_intervening_if_names_who_is_the_monarch(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::the_monarchs_end_step(
    "At the beginning of the monarch's end step, you gain 1 life.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"Monarch"}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::an_attack_at_the_monarch(
    "Whenever a creature attacks the monarch, put a +1/+1 counter on it.",
    r#"{"Triggered":{"when":{"AttacksPlayer":{"what":[{"Type":"Creature"}],"at":"Monarch"}},"effects":[{"PutCounters":{"on":"Triggering","counter":"+1/+1"}}]}}"#
)]
#[case::equipped_creature_attacks_the_monarch(
    "Whenever equipped creature attacks the monarch, destroy target tapped nonland permanent that player controls.",
    r#"{"Triggered":{"when":{"AttacksPlayer":{"what":[{"Is":"Attached"}],"at":"Monarch"}},"targets":[{"what":{"Object":["Tapped",{"Not":{"Type":"Land"}},{"ControlledBy":"Monarch"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#
)]
#[case::destroying_what_the_monarch_controls(
    "Whenever a creature attacks the monarch, destroy target creature the monarch controls.",
    r#"{"Triggered":{"when":{"AttacksPlayer":{"what":[{"Type":"Creature"}],"at":"Monarch"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Monarch"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#
)]
fn the_monarch_is_a_player_a_trigger_names(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::the_monarch_controls_the_blockers(
    "This creature can't be blocked by creatures the monarch controls.",
    r#"{"Static":{"affected":"This","mods":[{"CantBeBlockedBy":[{"Type":"Creature"},{"ControlledBy":"Monarch"}]}]}}"#
)]
#[case::as_long_as_after_a_bonus(
    "Creatures you control get +1/+1 as long as you're the monarch.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}],"condition":{"Monarch":"You"}}}"#
)]
#[case::as_long_as_after_a_keyword(
    "This creature has flying as long as an opponent is the monarch.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Flying"}],"condition":{"Monarch":"Opponents"}}}"#
)]
#[case::as_long_as_first(
    "As long as you're the monarch, permanents you control have hexproof.",
    r#"{"Static":{"affected":{"Each":[{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Hexproof"}],"condition":{"Monarch":"You"}}}"#
)]
#[case::as_long_as_first_for_the_attached_creature(
    "As long as there is no monarch, equipped creature gets +1/+1.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":1,"toughness":1}}],"condition":{"Not":{"Monarch":"EachPlayer"}}}}"#
)]
fn a_static_ability_reads_the_monarch(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::when_is_not_how_a_card_says_it("When you become the monarch, scry 1.")]
#[case::a_trigger_on_another_verb("Whenever you become the king, draw a card.")]
#[case::a_target_player_becomes_it("Whenever target player becomes the monarch, draw a card.")]
#[case::the_monarch_is_not_an_attacked_player("Whenever a creature attacks the king, draw a card.")]
fn near_misses_of_the_events_stay_with_the_table(#[case] text: &str) {
    assert_eq!(trigger(text), None);
}

#[rstest]
#[case::a_player_who_is_the_monarch(
    "Creatures you control get +1/+1 as long as a player is the monarch."
)]
#[case::the_monarch_as_a_blocker_is_a_restriction_under_a_condition(
    "This creature can't be blocked by creatures the monarch controls as long as you're the monarch."
)]
#[case::the_monarch_has_no_pronoun("Creatures the monarch has get +1/+1.")]
fn near_misses_of_the_statics_stay_with_the_table(#[case] text: &str) {
    assert_eq!(stat(text), None);
}

#[rstest]
#[case::no_monarch_is_not_a_life_total(
    "At the beginning of your upkeep, if there is a monarch, draw a card."
)]
#[case::an_unknown_state("At the beginning of your upkeep, if the monarch is dead, draw a card.")]
fn near_misses_of_the_conditions_stay_with_the_table(#[case] text: &str) {
    assert_eq!(trigger(text), None);
}

#[rstest]
#[case::your_commander_deals_combat_damage(
    "Whenever your commander deals combat damage to a player, you become the monarch.",
    r#"{"Triggered":{"when":{"DealsDamage":{"source":["Commander",{"OwnedBy":"You"}],"to":"EachPlayer","combat":true}},"effects":[{"BecomeMonarch":{}}]}}"#
)]
#[case::when_your_commander_and_no_monarch(
    "When your commander deals combat damage to a player, if there is no monarch, you become the monarch.",
    r#"{"Triggered":{"when":{"DealsDamage":{"source":["Commander",{"OwnedBy":"You"}],"to":"EachPlayer","combat":true}},"if":{"Not":{"Monarch":"EachPlayer"}},"effects":[{"BecomeMonarch":{}}]}}"#
)]
#[case::the_monarchs_end_step_names_that_player(
    "At the beginning of the monarch's end step, that player draws a card.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"Monarch"}},"effects":[{"Draw":{"who":"Triggering"}}]}}"#
)]
#[case::an_attack_at_an_opponent_who_is_the_monarch(
    "Whenever a creature attacks one of your opponents, if that player is the monarch, that creature gets +1/+1 and gains trample until end of turn.",
    r#"{"Triggered":{"when":{"AttacksPlayer":{"what":[{"Type":"Creature"}],"at":"Opponents"}},"if":{"Monarch":"Defending"},"effects":[{"Continuous":{"affected":"Triggering","mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Trample"}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::an_attack_at_an_opponent_with_it(
    "Whenever a creature attacks one of your opponents, it gets +1/+1 until end of turn.",
    r#"{"Triggered":{"when":{"AttacksPlayer":{"what":[{"Type":"Creature"}],"at":"Opponents"}},"effects":[{"Continuous":{"affected":"Triggering","mods":[{"ModifyPT":{"power":1,"toughness":1}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::was_the_monarch_as_the_turn_began(
    "Whenever an opponent becomes the monarch, if you were the monarch as the turn began, that player loses 2 life and you gain 2 life.",
    r#"{"Triggered":{"when":{"BecomesMonarch":{"who":"Opponents"}},"if":{"MonarchAtTurnStart":"You"},"effects":[{"LoseLife":{"who":"Triggering","amount":2}},{"GainLife":{"amount":2}}]}}"#
)]
#[case::an_opponent_was_the_monarch_as_the_turn_began(
    "Whenever you cast a spell, if an opponent was the monarch as the turn began, you gain 1 life.",
    r#"{"Triggered":{"when":{"SpellCast":{"by":"You"}},"if":{"MonarchAtTurnStart":"Opponents"},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::exile_until_an_opponent_becomes_the_monarch(
    "When ~ enters, exile target creature an opponent controls until an opponent becomes the monarch.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Exile":{"what":{"Target":0},"duration":"UntilOpponentBecomesMonarch"}}]}}"#
)]
#[case::the_monarch_held_after_an_event(
    "Whenever you tap a land for mana while you're the monarch, draw a card.",
    r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}],"by":"You"}},"if":{"Monarch":"You"},"effects":[{"Draw":{}}]}}"#
)]
#[case::an_opponent_the_monarch_after_an_event(
    "Whenever you cast a spell while an opponent is the monarch, you gain 1 life.",
    r#"{"Triggered":{"when":{"SpellCast":{"by":"You"}},"if":{"Monarch":"Opponents"},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
fn the_monarch_wordings_of_the_commander_the_attack_the_turn_the_exile_and_the_held_designation(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::while_the_monarch(
    "As long as you're the monarch, damage doesn't cause you to lose life.",
    r#"{"Rule":{"rule":{"NoLifeLossFromDamage":"You"},"condition":{"Monarch":"You"}}}"#
)]
#[case::the_condition_after(
    "Damage doesn't cause you to lose life as long as you're the monarch.",
    r#"{"Rule":{"rule":{"NoLifeLossFromDamage":"You"},"condition":{"Monarch":"You"}}}"#
)]
#[case::always(
    "Damage doesn't cause you to lose life.",
    r#"{"Rule":{"rule":{"NoLifeLossFromDamage":"You"}}}"#
)]
fn damage_that_causes_no_loss_of_life_is_a_rule_of_the_game(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::another_players_commander("Whenever target player's commander deals combat damage to a player, you become the monarch.")]
#[case::an_opponents_commander("Whenever an opponent's commander deals combat damage to a player, you become the monarch.")]
#[case::your_commanders_with_an_article("Whenever a your commander deals combat damage to a player, you become the monarch.")]
#[case::damage_to_you_while_the_monarch("Whenever a creature deals combat damage to you while you're the monarch, draw a card.")]
#[case::a_group_of_creatures_while_the_monarch("Whenever one or more creatures deal combat damage to you while you're the monarch, draw a card.")]
#[case::that_player_is_the_monarch_of_no_attack("Whenever you draw a card, if that player is the monarch, draw a card.")]
#[case::until_the_controller_becomes_it("When ~ enters, exile target creature an opponent controls until you become the monarch.")]
#[case::until_a_player_becomes_it("When ~ enters, exile target creature an opponent controls until a player becomes the monarch.")]
#[case::both_a_condition_and_a_tail("Whenever you cast a spell while you're the monarch, if an opponent is the monarch, draw a card.")]
fn near_misses_of_the_commander_the_attack_the_exile_and_the_tail_stay_with_the_table(
    #[case] text: &str,
) {
    assert_eq!(trigger(text), None);
}

#[rstest]
#[case::another_players_life("Damage doesn't cause target player to lose life.")]
#[case::damage_to_a_creature("Damage doesn't cause you to lose life if you're the monarch.")]
#[case::each_player("Damage doesn't cause players to lose life.")]
fn near_misses_of_the_rule_stay_with_the_table(#[case] text: &str) {
    assert_eq!(stat(text), None);
}
