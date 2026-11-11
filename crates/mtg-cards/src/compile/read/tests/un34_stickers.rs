//! Goal: the sticker and prize wordings (put a sticker on a permanent you own, claim the prize, the prize ability "Prize — ...", "whenever you visit an Attraction", "whenever you claim the prize of an Attraction" and the Attractions you've visited this turn) read into the DSL and write back as the card's own text, and every near miss stays with the table.
use super::*;

use crate::compile::read::visit_block;

#[rstest]
#[case::prize_wall(
    "{4}{U}, {T}: You may put a sticker on a nonland permanent you own. Activate only as a sorcery.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"Mana":"{4}{U}"},"Tap"],"timing":"Sorcery","effects":[{"May":{"effects":[{"PutSticker":{"on":[{"Not":{"Type":"Land"}},{"OwnedBy":"You"}]}}]}}]}}"#
)]
#[case::on_entering(
    "When ~ enters, put a sticker on a creature you own.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"PutSticker":{"on":[{"Type":"Creature"},{"OwnedBy":"You"}]}}]}}"#
)]
#[case::ticket_then_sticker(
    "When ~ enters, you get {TK}, then you may put a sticker on a nonland permanent you own.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"PutCounters":{"on":"You","counter":{"Other":"ticket"},"count":1}},{"May":{"effects":[{"PutSticker":{"on":[{"Not":{"Type":"Land"}},{"OwnedBy":"You"}]}}]}}]}}"#
)]
#[case::whenever_you_visit(
    "Whenever you visit an Attraction, you gain 1 life.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"VisitedAttraction":{"who":"You"}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::whenever_you_claim(
    "Whenever you claim the prize of an Attraction, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ClaimedPrize":{"who":"You"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::claim_on_entering(
    "When ~ enters, claim the prize.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"ClaimPrize":{}}]}}"#
)]
#[case::visited_count(
    "Draw cards equal to the number of Attractions you've visited this turn.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Draw":{"count":{"Tallied":{"what":"AttractionsVisited"}}}}]}}"#
)]
fn the_sticker_and_prize_wordings_read(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::draw("Prize \u{2014} Draw a card.", r#"[{"Draw":{}}]"#)]
#[case::open("Prize \u{2014} Open an Attraction.", r#"[{"OpenAttraction":{}}]"#)]
#[case::visit_claims("Visit \u{2014} Claim the prize.", r#"[{"ClaimPrize":{}}]"#)]
fn a_prize_line_is_a_prize_ability(#[case] text: &str, #[case] effects: &str) {
    let when = if text.starts_with("Prize") {
        r#"{"Prize":{}}"#
    } else {
        r#"{"Visit":{}}"#
    };
    let expected = ability(&format!(
        r#"{{"Triggered":{{"when":{when},"effects":{effects}}}}}"#
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
#[case::not_owned(
    "Put a sticker on a nonland permanent an opponent controls.",
    ClauseKind::Spell
)]
#[case::two_stickers(
    "Put two stickers on a nonland permanent you own.",
    ClauseKind::Spell
)]
#[case::no_object("Put a sticker.", ClauseKind::Spell)]
#[case::visits_two(
    "Whenever you visit two Attractions, draw a card.",
    ClauseKind::Triggered
)]
#[case::prize_of_nothing(
    "Whenever you claim the prize, draw a card.",
    ClauseKind::Triggered
)]
#[case::a_prize("Claim a prize.", ClauseKind::Spell)]
fn a_near_miss_is_not_read(#[case] text: &str, #[case] kind: ClauseKind) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}

#[rstest]
#[case::no_dash("Prize Draw a card.")]
#[case::unread("Prize \u{2014} Frobnicate the widgets.")]
#[case::empty("Prize \u{2014} ")]
fn a_prize_line_the_reader_does_not_know_stays_with_the_table(#[case] text: &str) {
    assert_eq!(visit_block(text, false, &Own::default()), None, "{text}");
}
