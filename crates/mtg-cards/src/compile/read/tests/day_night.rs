//! Goal: the day and night wordings ("It becomes night.", "If it's night, it becomes day. Otherwise, it becomes night.", "if it's day", "if it's neither day nor night", "Whenever day becomes night or night becomes day", "If it's neither day nor night, it becomes day as ~ enters.") read into the DSL, write back as the card's own text, and leave the near-misses to the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::either_change(
    "Whenever day becomes night or night becomes day, draw a card.",
    r#"{"Triggered":{"when":{"DayNightChanges":{}},"effects":[{"Draw":{}}]}}"#
)]
#[case::night_becomes_day(
    "Whenever night becomes day, you gain 1 life.",
    r#"{"Triggered":{"when":{"DayNightChanges":{"to":"Day"}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::day_becomes_night(
    "Whenever day becomes night, draw a card.",
    r#"{"Triggered":{"when":{"DayNightChanges":{"to":"Night"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::with_a_target(
    "Whenever day becomes night or night becomes day, put a +1/+1 counter on target creature you control.",
    r#"{"Triggered":{"when":{"DayNightChanges":{}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}}]}}"#
)]
fn a_change_between_day_and_night_is_an_event(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::night(
    "It becomes night.",
    r#"{"Spell":{"effects":[{"BecomeDayNight":{"to":"Night"}}]}}"#
)]
#[case::day(
    "It becomes day.",
    r#"{"Spell":{"effects":[{"BecomeDayNight":{"to":"Day"}}]}}"#
)]
#[case::the_toggle(
    "If it's night, it becomes day. Otherwise, it becomes night.",
    r#"{"Spell":{"effects":[{"If":{"condition":"IsNight","then":[{"BecomeDayNight":{"to":"Day"}}],"otherwise":[{"BecomeDayNight":{"to":"Night"}}]}}]}}"#
)]
#[case::the_toggle_from_day(
    "If it's day, it becomes night. Otherwise, it becomes day.",
    r#"{"Spell":{"effects":[{"If":{"condition":"IsDay","then":[{"BecomeDayNight":{"to":"Night"}}],"otherwise":[{"BecomeDayNight":{"to":"Day"}}]}}]}}"#
)]
fn a_spell_makes_it_day_or_night(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[test]
fn the_toggle_is_an_activated_ability_at_sorcery_speed() {
    reads_as(
        "{3}, {T}: If it's night, it becomes day. Otherwise, it becomes night. Activate only as a sorcery.",
        ClauseKind::Activated,
        false,
        r#"{"Activated":{"cost":[{"Mana":"{3}"},"Tap"],"effects":[{"If":{"condition":"IsNight","then":[{"BecomeDayNight":{"to":"Day"}}],"otherwise":[{"BecomeDayNight":{"to":"Night"}}]}}],"timing":"Sorcery"}}"#,
    );
}

#[test]
fn a_permanent_starts_the_day_as_it_enters() {
    reads_as(
        "If it's neither day nor night, it becomes day as ~ enters.",
        ClauseKind::Static,
        false,
        r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":"DayIfNeither"}}"#,
    );
}

#[rstest]
#[case::the_night(
    "At the beginning of your upkeep, if it's night, draw a card.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":"IsNight","effects":[{"Draw":{}}]}}"#
)]
#[case::the_day(
    "At the beginning of your upkeep, if it's day, you gain 1 life.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":"IsDay","effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::neither(
    "When ~ enters, if it's neither day nor night, draw a card.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":"NeitherDayNorNight","effects":[{"Draw":{}}]}}"#
)]
fn an_intervening_if_names_how_the_designation_stands(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::dusk("It becomes dusk.")]
#[case::the_game("The game becomes night.")]
#[case::a_player("Target player becomes night.")]
#[case::both("It becomes day and night.")]
#[case::a_toggle_with_no_otherwise("If it's night, it becomes day.")]
#[case::a_toggle_from_neither(
    "If it's neither day nor night, it becomes day. Otherwise, it becomes night."
)]
#[case::a_toggle_that_does_something_else("If it's night, it becomes day. Otherwise, draw a card.")]
fn near_misses_of_the_spells_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Spell, false), None);
}

#[rstest]
#[case::day_only("Whenever day becomes night or night, draw a card.")]
#[case::dusk("Whenever day becomes dusk, draw a card.")]
#[case::it_becomes_night("Whenever it becomes night, draw a card.")]
#[case::night_is_no_monarch("Whenever night becomes the monarch, draw a card.")]
fn near_misses_of_the_events_stay_with_the_table(#[case] text: &str) {
    assert_eq!(trigger(text), None);
}

#[rstest]
#[case::night_at_enter("If it's night, it becomes day as ~ enters.")]
#[case::night_instead("If it's neither day nor night, it becomes night as ~ enters.")]
#[case::as_another_card(
    "If it's neither day nor night, it becomes day as another creature enters."
)]
fn near_misses_of_the_statics_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None);
}

#[rstest]
#[case::nighttime("At the beginning of your upkeep, if it's nighttime, draw a card.")]
#[case::dawn("At the beginning of your upkeep, if it's dawn, draw a card.")]
fn near_misses_of_the_conditions_stay_with_the_table(#[case] text: &str) {
    assert_eq!(trigger(text), None);
}
