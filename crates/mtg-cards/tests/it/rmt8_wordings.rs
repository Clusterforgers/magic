//! Goal: the writers of pack rmt8 word each shape as the Oracle text does (a replacement under a condition, a target
//! chosen at random, life for each target, a spell that targets only a single object of a kind, a tap of creatures of
//! either of two colors, the caster's first turns, an attack on a battle, other creatures with morph abilities, a turn
//! face down, a harnessed stone's ∞ ability, Curses attached to you, another target player, a creature that evolves)
//! another player who loses the game with their life total as the turn began, a counter for fighting since your last
//! upkeep) and refuse the near misses.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::written_variants;
use rstest::rstest;

/// "It's your first, second, or third turn of the game".
const EARLY: &str =
    r#"{"All":["YourTurn",{"Compare":{"left":{"TurnsTaken":"You"},"op":"Le","right":3}}]}"#;

fn says(json: &str, text: &str) -> bool {
    let ability: Ability = serde_json::from_str(json).unwrap();
    written_variants(&ability).iter().any(|said| said == text)
}

#[rstest]
#[case::while_condition(
    r#"{"Replacement":{"when":{"Damage":{"source":[{"ControlledBy":"You"}],"combat":false}},"by":{"Multiply":2},"condition":{"Compare":{"left":{"CardTypes":[{"In":"Graveyard"},{"OwnedBy":"You"}]},"op":"Ge","right":4}}}}"#,
    "If a source you control would deal noncombat damage to a permanent or player while there are four or more card types among cards in your graveyard, it deals double that damage instead.",
    true
)]
#[case::while_needs_the_condition(
    r#"{"Replacement":{"when":{"Damage":{"source":[{"ControlledBy":"You"}],"combat":false}},"by":{"Multiply":2}}}"#,
    "If a source you control would deal noncombat damage to a permanent or player while there are four or more card types among cards in your graveyard, it deals double that damage instead.",
    false
)]
#[case::as_long_as(
    r#"{"Replacement":{"when":{"Enters":{"what":["Other",{"Subtype":"Vampire"},{"ControlledBy":"You"}]}},"by":{"WithCounters":{"counter":"+1/+1","count":1}},"condition":{"Compare":{"left":{"Tallied":{"who":"Opponents","what":"LifeLost"}},"op":"Ge","right":1}}}}"#,
    "As long as an opponent lost life this turn, each other Vampire you control enters with an additional +1/+1 counter on it.",
    true
)]
#[case::as_long_as_another_condition(
    r#"{"Replacement":{"when":{"Enters":{"what":["Other",{"Subtype":"Vampire"},{"ControlledBy":"You"}]}},"by":{"WithCounters":{"counter":"+1/+1","count":1}},"condition":{"Compare":{"left":{"Tallied":{"who":"You","what":"LifeLost"}},"op":"Ge","right":1}}}}"#,
    "As long as an opponent lost life this turn, each other Vampire you control enters with an additional +1/+1 counter on it.",
    false
)]
#[case::at_random(
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"targets":[{"what":{"AtRandom":"Opponent"}}],"effects":[{"Continuous":{"affected":"This","mods":[{"SetController":{"Target":0}}],"duration":"Forever"}}]}}"#,
    "At the beginning of your end step, target opponent chosen at random gains control of ~.",
    true
)]
#[case::chosen_target_is_not_random(
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"targets":[{"what":"Opponent"}],"effects":[{"Continuous":{"affected":"This","mods":[{"SetController":{"Target":0}}],"duration":"Forever"}}]}}"#,
    "At the beginning of your end step, target opponent chosen at random gains control of ~.",
    false
)]
#[case::life_for_each_target(
    r#"{"CostModifier":{"applies_to":["This"],"change":"Increase","mana":"","per":{"TargetCount":0},"life":3}}"#,
    "This spell costs 3 life more to cast for each target.",
    true
)]
#[case::life_once(
    r#"{"CostModifier":{"applies_to":["This"],"change":"Increase","mana":"","life":3}}"#,
    "This spell costs 3 life more to cast for each target.",
    false
)]
#[case::single_creature(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":["Spell","SingleTarget",{"Targeting":[{"Type":"Creature"}]}]}}],"effects":[{"ChangeTargets":{"what":{"Target":0},"to":"One","onto":"This"}}]}}"#,
    "When ~ enters, change the target of target spell that targets only a single creature to ~.",
    true
)]
#[case::single_target_of_any_kind(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":["Spell","SingleTarget"]}}],"effects":[{"ChangeTargets":{"what":{"Target":0},"to":"One","onto":"This"}}]}}"#,
    "When ~ enters, change the target of target spell that targets only a single creature to ~.",
    false
)]
#[case::either_color_tapped(
    r#"{"Activated":{"cost":[{"TapPermanents":{"filter":[{"Type":"Creature"},{"AnyOf":[{"Color":"White"},{"Color":"Blue"}]}],"count":2}}],"effects":[{"Draw":{}}]}}"#,
    "Tap two untapped white and/or blue creatures you control: Draw a card.",
    true
)]
#[case::one_color_tapped(
    r#"{"Activated":{"cost":[{"TapPermanents":{"filter":[{"Type":"Creature"},{"Color":"White"}],"count":2}}],"effects":[{"Draw":{}}]}}"#,
    "Tap two untapped white and/or blue creatures you control: Draw a card.",
    false
)]
#[case::attacks_a_battle(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This","AttackingBattle"]}},"effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":1}}],"duration":"UntilEndOfTurn"}}]}}"#,
    "Whenever ~ attacks a battle, it gets +1/+1 until end of turn.",
    true
)]
#[case::attacks_anything(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":1}}],"duration":"UntilEndOfTurn"}}]}}"#,
    "Whenever ~ attacks a battle, it gets +1/+1 until end of turn.",
    false
)]
#[case::with_morph(
    r#"{"Triggered":{"when":{"TurnedFaceUp":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"AnyOf":[{"HasKeywordNamed":"Morph"},{"HasKeywordNamed":"Megamorph"}]},"Other"]},"bounds":{"min":0,"max":null}}],"effects":[{"TurnFaceDown":{"what":{"Target":0}}}]}}"#,
    "When ~ is turned face up, turn any number of target creatures with morph abilities other than ~ face down.",
    true
)]
#[case::without_megamorph(
    r#"{"Triggered":{"when":{"TurnedFaceUp":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"HasKeywordNamed":"Morph"},"Other"]},"bounds":{"min":0,"max":null}}],"effects":[{"TurnFaceDown":{"what":{"Target":0}}}]}}"#,
    "When ~ is turned face up, turn any number of target creatures with morph abilities other than ~ face down.",
    false
)]
#[case::turn_face_down(
    r#"{"Activated":{"cost":"{3}","effects":[{"TurnFaceDown":{"what":"This"}}]}}"#,
    "{3}: Turn ~ face down.",
    true
)]
#[case::turn_face_up_is_another_text(
    r#"{"Activated":{"cost":"{3}","effects":[{"TurnFaceUp":{"what":"This"}}]}}"#,
    "{3}: Turn ~ face down.",
    false
)]
#[case::infinity(
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Matches":{"subject":"This","selector":["Harnessed"]}},"effects":[{"GainLife":{"amount":1}}]}}"#,
    "\u{221e} \u{2014} At the beginning of your upkeep, you gain 1 life.",
    true
)]
#[case::infinity_needs_the_harness(
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"GainLife":{"amount":1}}]}}"#,
    "\u{221e} \u{2014} At the beginning of your upkeep, you gain 1 life.",
    false
)]
#[case::harness(
    r#"{"Activated":{"cost":"{0}","effects":[{"Harness":{"what":"This"}}]}}"#,
    "{0}: Harness ~.",
    true
)]
#[case::attached_to_you(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Destroy":{"what":{"Each":[{"Subtype":"Curse"},{"AttachedTo":"You"}]}}}]}}"#,
    "When ~ enters, destroy all Curses attached to you.",
    true
)]
#[case::every_curse(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Destroy":{"what":{"Each":[{"Subtype":"Curse"}]}}}]}}"#,
    "When ~ enters, destroy all Curses attached to you.",
    false
)]
#[case::another_player(
    r#"{"Triggered":{"when":{"Nth":{"event":{"SpellCast":{}},"nth":1}},"targets":[{"what":{"PlayerAmong":{"PlayersWhere":{"who":"EachPlayer","test":{"Not":{"Among":"TriggeringPlayer"}}}}}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}}]}}"#,
    "Whenever a player casts their first spell each turn, choose another target player. ~ deals damage equal to that spell's mana value to the chosen player.",
    true
)]
#[case::any_player(
    r#"{"Triggered":{"when":{"Nth":{"event":{"SpellCast":{}},"nth":1}},"targets":[{"what":"Player"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}}]}}"#,
    "Whenever a player casts their first spell each turn, choose another target player. ~ deals damage equal to that spell's mana value to the chosen player.",
    false
)]
#[case::evolves(
    r#"{"Triggered":{"when":{"Did":{"action":"Evolved","what":["This"]}},"effects":[{"GainLife":{"amount":1}}]}}"#,
    "Whenever ~ evolves, you gain 1 life.",
    true
)]
#[case::adapts_is_no_evolve(
    r#"{"Triggered":{"when":{"Did":{"action":"Adapted","what":["This"]}},"effects":[{"GainLife":{"amount":1}}]}}"#,
    "Whenever ~ evolves, you gain 1 life.",
    false
)]
#[case::another_player_loses(
    r#"{"Triggered":{"when":{"Did":{"action":"LostGame","who":{"PlayersWhere":{"who":"EachPlayer","test":{"Not":{"Among":"Asker"}}}}}},"effects":[{"GainLife":{"amount":{"LifeAsTurnBegan":"TriggeringPlayer"}}}]}}"#,
    "Whenever another player loses the game, you gain life equal to that player's life total as the turn began.",
    true
)]
#[case::their_life_now(
    r#"{"Triggered":{"when":{"Did":{"action":"LostGame","who":{"PlayersWhere":{"who":"EachPlayer","test":{"Not":{"Among":"Asker"}}}}}},"effects":[{"GainLife":{"amount":{"Life":"TriggeringPlayer"}}}]}}"#,
    "Whenever another player loses the game, you gain life equal to that player's life total as the turn began.",
    false
)]
#[case::an_opponent_is_not_another_player(
    r#"{"Triggered":{"when":{"Did":{"action":"LostGame","who":"Opponents"}},"effects":[{"GainLife":{"amount":{"LifeAsTurnBegan":"TriggeringPlayer"}}}]}}"#,
    "Whenever another player loses the game, you gain life equal to that player's life total as the turn began.",
    false
)]
#[case::attacked_or_blocked(
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"If":{"condition":{"Matches":{"subject":"Attached","selector":[{"FoughtSinceYourLastUpkeep":{"attacked":true,"blocked":true}}]}},"then":[{"PutCounters":{"on":"Attached","counter":"+1/+1"}}],"otherwise":[{"RemoveCounters":{"from":"Attached","counter":"+1/+1"}}]}}]}}"#,
    "At the beginning of your upkeep, put a +1/+1 counter on enchanted creature if it attacked or blocked since your last upkeep. Otherwise, remove a +1/+1 counter from it.",
    true
)]
#[case::blocked_or_been_blocked(
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"If":{"condition":{"Matches":{"subject":"This","selector":[{"FoughtSinceYourLastUpkeep":{"blocked":true,"was_blocked":true}}]}},"then":[{"PutCounters":{"on":"This","counter":"+1/+1"}}],"otherwise":[{"RemoveCounters":{"from":"This","counter":"+1/+1"}}]}}]}}"#,
    "At the beginning of your upkeep, put a +1/+1 counter on ~ if it has blocked or been blocked since your last upkeep. Otherwise, remove a +1/+1 counter from it.",
    true
)]
#[case::attacked_alone_is_another_text(
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"If":{"condition":{"Matches":{"subject":"Attached","selector":[{"FoughtSinceYourLastUpkeep":{"attacked":true}}]}},"then":[{"PutCounters":{"on":"Attached","counter":"+1/+1"}}],"otherwise":[{"RemoveCounters":{"from":"Attached","counter":"+1/+1"}}]}}]}}"#,
    "At the beginning of your upkeep, put a +1/+1 counter on enchanted creature if it attacked or blocked since your last upkeep. Otherwise, remove a +1/+1 counter from it.",
    false
)]
fn a_shape_reads_back_as_its_text(#[case] json: &str, #[case] text: &str, #[case] said: bool) {
    assert_eq!(says(json, text), said);
}

#[rstest]
#[case::cast(
    format!(r#"{{"CastOnly":{{"condition":{{"Not":{EARLY}}}}}}}"#),
    "You can't cast ~ during your first, second, or third turns of the game.",
    true
)]
#[case::cast_in_two_turns(
    format!(r#"{{"CastOnly":{{"condition":{{"Not":{}}}}}}}"#, EARLY.replace("\"right\":3", "\"right\":2")),
    "You can't cast ~ during your first, second, or third turns of the game.",
    false
)]
#[case::enters_tapped(
    format!(r#"{{"Replacement":{{"when":{{"Enters":{{"what":["This"]}}}},"by":"Tapped","condition":{{"Not":{EARLY}}}}}}}"#),
    "~ enters tapped unless it's your first, second, or third turn of the game.",
    true
)]
#[case::any_turn_is_not_yours(
    format!(r#"{{"Replacement":{{"when":{{"Enters":{{"what":["This"]}}}},"by":"Tapped","condition":{{"Not":{}}}}}}}"#, EARLY.replace("\"YourTurn\",", "")),
    "~ enters tapped unless it's your first, second, or third turn of the game.",
    false
)]
fn the_first_turns_read_back(#[case] json: String, #[case] text: &str, #[case] said: bool) {
    assert_eq!(says(&json, text), said);
}
