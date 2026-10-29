//! Goal: the dungeon wordings (venture into the dungeon as an effect, "whenever you venture into the dungeon", "whenever you complete a dungeon" and "if you've completed a dungeon") read into the DSL and write back as the card's own text.
use super::*;

#[rstest]
#[case::venture_on_entering(
    "When ~ enters, venture into the dungeon.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Venture":{}}]}}"#
)]
#[case::venture_on_a_hit(
    "Whenever one or more creatures you control deal combat damage to a player, venture into the dungeon.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Batch":{"DealsDamage":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"EachPlayer","combat":true}}},"effects":[{"Venture":{}}]}}"#
)]
#[case::venture_as_a_cost(
    "{3}, {T}: Venture into the dungeon. Activate only as a sorcery.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"Mana":"{3}"},"Tap"],"timing":"Sorcery","effects":[{"Venture":{}}]}}"#
)]
#[case::whenever_you_venture(
    "Whenever you venture into the dungeon, you gain 1 life.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Ventured":{"who":"You"}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::whenever_you_complete_a_dungeon(
    "Whenever you complete a dungeon, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"CompletedDungeon":{"who":"You"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::if_you_completed_a_dungeon(
    "At the beginning of your upkeep, if you've completed a dungeon, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"CompletedDungeon":"You"},"effects":[{"Draw":{}}]}}"#
)]
fn the_dungeon_wordings_read(#[case] text: &str, #[case] kind: ClauseKind, #[case] json: &str) {
    reads_as(text, kind, false, json);
}
