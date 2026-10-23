//! Goal: a triggered ability in quotes that a static ability gives ("has \"Whenever this creature attacks, ...\"", "Sliver creatures you control have \"When this creature enters, ...\"") reads into a `GrantAbility`, also beside a bonus and under a leading condition (Lieutenant), writes back as the card's own text, and a quoted ability the reader does not know stays with the table.
use super::*;

/// "As long as you control your commander" as a condition.
const COMMANDER: &str = r#"{"Compare":{"left":{"Count":["Commander",{"OwnedBy":"You"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#;

fn lieutenant(granted: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"ModifyPT":{{"power":2,"toughness":2}}}},{{"GrantAbility":{granted}}}],"condition":{COMMANDER}}}}}"#
    )
}

fn group(preds: &str, granted: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":{{"Each":{preds}}},"mods":[{{"GrantAbility":{granted}}}]}}}}"#
    )
}

#[rstest]
#[case::sacrifice(
    "As long as you control your commander, ~ gets +2/+2 and has \"Whenever ~ deals combat damage to a player, that player sacrifices a creature of their choice.\"",
    lieutenant(r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"Sacrifice":{"who":"Triggering","filter":[{"Type":"Creature"}]}}]}}"#)
)]
#[case::damage_to_a_creature_of_the_defending_player(
    "As long as you control your commander, ~ gets +2/+2 and has \"Whenever ~ attacks, it deals 7 damage to target creature defending player controls.\"",
    lieutenant(r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Defending"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":7}}]}}"#)
)]
#[case::draw_when_blocked(
    "As long as you control your commander, ~ gets +2/+2 and has \"Whenever ~ becomes blocked, you may draw two cards.\"",
    lieutenant(r#"{"Triggered":{"when":{"BecomesBlocked":{"what":["This"]}},"effects":[{"May":{"effects":[{"Draw":{"count":2}}]}}]}}"#)
)]
#[case::every_creature_you_control(
    "Creatures you control have \"Whenever ~ attacks, draw a card.\"",
    group(
        r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#,
        r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Draw":{}}]}}"#
    )
)]
#[case::every_sliver(
    "All Slivers have \"When ~ enters, destroy target artifact or enchantment.\"",
    group(r#"[{"Subtype":"Sliver"}]"#, r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#)
)]
#[case::slivers_you_control(
    "Sliver creatures you control have \"When ~ enters, it deals 1 damage to target player or planeswalker and you gain 1 life.\"",
    group(r#"[{"Type":"Creature"},{"Subtype":"Sliver"},{"ControlledBy":"You"}]"#, r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Either":["Player",{"Object":[{"Type":"Planeswalker"}]}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}},{"GainLife":{"amount":1}}]}}"#)
)]
fn a_granted_triggered_ability_reads_and_writes_back(#[case] text: &str, #[case] json: String) {
    reads_as(text, ClauseKind::Static, false, &json);
}

#[rstest]
#[case::the_ability_word_is_flavor(
    "Lieutenant \u{2014} As long as you control your commander, ~ gets +2/+2 and has \"Whenever ~ becomes blocked, you may draw two cards.\"",
    "As long as you control your commander, ~ gets +2/+2 and has \"Whenever ~ becomes blocked, you may draw two cards.\""
)]
fn the_ability_word_changes_nothing(#[case] with: &str, #[case] without: &str) {
    let read = clause(with, ClauseKind::Static, false);
    assert!(read.is_some(), "{with}");
    assert_eq!(read, clause(without, ClauseKind::Static, false), "{with}");
}

#[rstest]
#[case::an_effect_the_reader_does_not_know(
    "Creatures you control have \"Whenever ~ attacks, frobnicate.\""
)]
#[case::a_spell_effect_is_not_an_ability("Creatures you control have \"Draw a card.\"")]
#[case::a_trigger_that_is_not_closed(
    "Creatures you control have \"Whenever ~ attacks, draw a card."
)]
#[case::two_quoted_abilities(
    "Creatures you control have \"Whenever ~ attacks, draw a card.\" and \"When ~ enters, draw a card.\""
)]
#[case::the_event_is_not_read(
    "Creatures you control have \"Whenever the moon is full, draw a card.\""
)]
fn a_near_miss_is_left_to_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
