//! Goal: the initiative wordings ("you take the initiative" alone, after an entering trigger and joined to another effect, and "if you have the initiative") read into the DSL and write back as the card's own text.
use super::*;

#[rstest]
#[case::take_on_entering(
    "When this creature enters, you take the initiative.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"TakeInitiative":{}}]}}"#
)]
#[case::take_as_a_spell(
    "You take the initiative.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"TakeInitiative":{}}]}}"#
)]
#[case::if_you_have_the_initiative(
    "At the beginning of your upkeep, if you have the initiative, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Initiative":"You"},"effects":[{"Draw":{}}]}}"#
)]
fn the_initiative_wordings_read(#[case] text: &str, #[case] kind: ClauseKind, #[case] json: &str) {
    reads_as(text, kind, false, json);
}
