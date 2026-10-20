//! Goal: the older wording "enters the battlefield" reads as "enters", alone and beside "or attacks", for the card and for an object phrase, and is written as "enters"; the words that go on after it ("tapped", "under your control") stay with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::the_card(
    "When ~ enters the battlefield, draw a card.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::this_creature(
    "When this creature enters the battlefield, you gain 2 life.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"GainLife":{"amount":2}}]}}"#
)]
#[case::or_attacks(
    "Whenever ~ enters the battlefield or attacks, draw a card.",
    r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"Attacks":{"what":["This"]}}]},"effects":[{"Draw":{}}]}}"#
)]
#[case::or_dies(
    "When ~ enters the battlefield or dies, scry 1.",
    r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}}]},"effects":[{"Scry":{}}]}}"#
)]
#[case::another_creature(
    "Whenever another creature enters the battlefield, you gain 1 life.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other"],"to":"Battlefield"}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::a_creature_you_control(
    "Whenever a creature you control enters the battlefield, draw a card.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"Battlefield"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::the_same_wording_as_enters(
    "When ~ enters, draw a card.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{}}]}}"#
)]
fn the_older_wording_reads_into_the_same_pattern_as_enters(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case("When ~ enters the battlefield, draw two cards.")]
#[case("When ~ enters the battlefield, you may draw a card.")]
#[case("When ~ enters the battlefield, destroy target artifact.")]
#[case("When ~ enters the battlefield, return target card from your graveyard to your hand.")]
#[case("When ~ enters the battlefield, create a Treasure token.")]
#[case("When ~ enters the battlefield, each opponent loses 2 life.")]
#[case("When ~ enters the battlefield, target opponent discards a card and loses 2 life.")]
#[case("Whenever another creature you control enters the battlefield, you gain 1 life.")]
#[case("Whenever an artifact you control enters the battlefield, scry 1.")]
#[case("Whenever a land you control enters the battlefield, draw a card.")]
#[case("Landfall \u{2014} Whenever a land you control enters the battlefield, you gain 1 life.")]
#[case("When this artifact enters the battlefield, you gain 3 life.")]
#[case("When this Aura enters the battlefield, scry 1.")]
fn more_oracle_wordings_with_the_older_enters_read_and_write_back(#[case] text: &str) {
    let rest = text.split_once(" \u{2014} ").map_or(text, |(_, rest)| rest);
    assert!(trigger(text).is_some(), "{text}");
    let read = golden(rest, ClauseKind::Triggered, false);
    let written = render_ability(&read).unwrap_or_default();
    assert!(!written.contains("the battlefield"), "{written}");
}

#[rstest]
#[case::tapped("When ~ enters the battlefield tapped, draw a card.")]
#[case::under_your_control(
    "Whenever a creature enters the battlefield under your control, scry 1."
)]
#[case::under_an_opponents_control(
    "Whenever a creature enters the battlefield under an opponent's control, scry 1."
)]
#[case::with_counters("When ~ enters the battlefield with a +1/+1 counter, draw a card.")]
#[case::half_of_it("When ~ enters the, draw a card.")]
#[case::without_the("When ~ enters battlefield, draw a card.")]
#[case::another_zone("When ~ enters the graveyard, draw a card.")]
#[case::enter_in_the_plural("Whenever creatures enter the battlefield, draw a card.")]
#[case::twice("When ~ enters the battlefield the battlefield, draw a card.")]
fn words_that_go_on_after_the_older_enters_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[test]
fn leaving_the_battlefield_and_entering_it_are_different_events() {
    let leaves = trigger("When ~ leaves the battlefield, draw a card.");
    let enters = trigger("When ~ enters the battlefield, draw a card.");
    assert!(leaves.is_some() && enters.is_some());
    assert_ne!(leaves, enters);
}
