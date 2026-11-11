//! Goal: the Attraction wordings (open an Attraction, open a number of them, roll to visit your Attractions, the two "whenever you" events and the visit ability "Visit — ...") read into the DSL and write back as the card's own text, and every near miss stays with the table.
use super::*;

use crate::compile::read::visit_block;

#[rstest]
#[case::open_on_entering(
    "When ~ enters, open an Attraction.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"OpenAttraction":{}}]}}"#
)]
#[case::open_two(
    "Open two Attractions.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"OpenAttraction":{"count":2}}]}}"#
)]
#[case::roll_on_entering(
    "When ~ enters, roll to visit your Attractions.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"RollToVisit":{}}]}}"#
)]
#[case::whenever_you_open(
    "Whenever you open an Attraction, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"OpenedAttraction":{"who":"You"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::whenever_you_roll_to_visit(
    "Whenever you roll to visit your Attractions, you gain 1 life.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"RolledToVisit":{"who":"You"}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
fn the_attraction_wordings_read(#[case] text: &str, #[case] kind: ClauseKind, #[case] json: &str) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::draw("Visit \u{2014} Draw a card.", r#"[{"Draw":{}}]"#)]
#[case::scry("Visit \u{2014} Scry 1.", r#"[{"Scry":{"count":1}}]"#)]
#[case::open("Visit \u{2014} Open an Attraction.", r#"[{"OpenAttraction":{}}]"#)]
fn a_visit_line_is_a_visit_ability(#[case] text: &str, #[case] effects: &str) {
    let expected = ability(&format!(
        r#"{{"Triggered":{{"when":{{"Visit":{{}}}},"effects":{effects}}}}}"#
    ));
    let read = visit_block(text, false, &Own::default());
    assert_eq!(read, Some(expected), "{text}");
    assert_eq!(
        crate::compile::classify::classify(text, false),
        ClauseKind::Triggered,
        "{text}"
    );
    let written = render_variants(&read.unwrap_or_else(|| unreachable!()));
    assert!(
        written.iter().any(|found| same_text(found, text)),
        "{text}: {written:?}"
    );
}

#[rstest]
#[case::two_of_one("Open two Attraction.", ClauseKind::Spell)]
#[case::one_of_many("Open an Attractions.", ClauseKind::Spell)]
#[case::another_thing("Open a Door.", ClauseKind::Spell)]
#[case::their_attractions("Roll to visit their Attractions.", ClauseKind::Spell)]
#[case::visit_without_roll("Visit your Attractions.", ClauseKind::Spell)]
#[case::opens_two(
    "Whenever you open two Attractions, draw a card.",
    ClauseKind::Triggered
)]
fn a_near_miss_is_not_read(#[case] text: &str, #[case] kind: ClauseKind) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}

#[rstest]
#[case::no_dash("Visit Draw a card.")]
#[case::unread("Visit \u{2014} Frobnicate the widgets.")]
#[case::half_read("Visit \u{2014} Draw a card. Frobnicate the widgets.")]
#[case::empty("Visit \u{2014} ")]
fn a_visit_line_the_reader_does_not_know_stays_with_the_table(#[case] text: &str) {
    assert_eq!(visit_block(text, false, &Own::default()), None, "{text}");
}
