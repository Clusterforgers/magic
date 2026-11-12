//! Goal: a by-hand clause that is one triggered line whose words before the comma read carries that event as `when`, whatever its effect says, and nothing else does (a static, an activated or a spell clause, a trigger whose event does not read, a clause of several lines); `when` is never serialised, so a definition with it writes and hashes as one without it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ManualClause;
use mtg_cards::compile::by_hand_event;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::{Step, TriggerPattern};
use rstest::rstest;

fn names() -> Vec<String> {
    vec!["Odd Watcher".to_owned()]
}

#[rstest]
#[case::enters(
    "When Odd Watcher enters, flip a coin until you lose a flip.",
    Some(TriggerPattern::ZoneChange { what: Selector::this(), from: None, to: Some(ZoneKind::Battlefield) })
)]
#[case::upkeep(
    "At the beginning of your upkeep, each player votes for a card name.",
    Some(TriggerPattern::StepBegins { step: Step::Upkeep, whose: Ref::You })
)]
#[case::opponent_casts(
    "Whenever an opponent casts a spell, you may ask them a riddle.",
    Some(TriggerPattern::SpellCast { what: Selector::default(), by: Ref::Opponents })
)]
#[case::ability_word(
    "Landfall \u{2014} Whenever a land you control enters, juggle.",
    Some(by_hand_event("Whenever a land you control enters, juggle.", &names()).unwrap())
)]
#[case::static_clause("Creatures you control have hexproof as long as it rains.", None)]
#[case::activated("{T}: Ask a riddle.", None)]
#[case::spell("Draw a card for each riddle you know.", None)]
#[case::unread_event("Whenever the moon is full, you win the game.", None)]
#[case::two_lines(
    "When Odd Watcher enters, flip a coin.\nWhenever you gain life, juggle.",
    None
)]
#[case::empty("", None)]
fn a_triggered_by_hand_line_carries_the_event_its_words_read(
    #[case] text: &str,
    #[case] expected: Option<TriggerPattern>,
) {
    assert_eq!(by_hand_event(text, &names()), expected, "{text}");
    let clause =
        ManualClause::new(text, "you apply this triggered ability by hand").reading_when(&names());
    assert_eq!(clause.when, expected);
}

#[test]
fn the_event_is_never_written() {
    let text = "When Odd Watcher enters, flip a coin until you lose a flip.";
    let plain = ManualClause::new(text, "by hand");
    let read = plain.clone().reading_when(&names());
    assert!(read.when.is_some());
    assert_eq!(
        serde_json::to_string(&read).unwrap(),
        serde_json::to_string(&plain).unwrap()
    );
    let back: ManualClause = serde_json::from_str(&serde_json::to_string(&read).unwrap()).unwrap();
    assert_eq!(back, plain);
}
