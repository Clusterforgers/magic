//! Goal: a player or a player and their permanents becoming the target of a spell or ability, and the keyword actions a player performs (investigate, discover, manifest dread, give a gift), read into their patterns and write back as the card's text; other subjects and verbs stay elsewhere.
use super::*;
use crate::trigger::TriggerPattern;

/// The pair of "you or a permanent you control becomes the target of" with `by` as the targeting selector.
fn pair(by: &str) -> String {
    format!(
        r#"{{"AnyOf":[{{"PlayerBecomesTarget":{{"who":"You","by":{by}}}}},{{"BecomesTarget":{{"what":[{{"ControlledBy":"You"}}],"by":{by}}}}}]}}"#
    )
}

#[rstest]
#[case::leovold(
    "Whenever you or a permanent you control becomes the target of a spell or ability an opponent controls, you may draw a card.",
    &pair(r#"[{"ControlledBy":"Opponents"}]"#),
    r#"[{"May":{"effects":[{"Draw":{}}]}}]"#
)]
#[case::mariner(
    "Whenever you or a permanent you control becomes the target of a spell or ability an opponent controls, counter that spell or ability unless its controller pays {1}.",
    &pair(r#"[{"ControlledBy":"Opponents"}]"#),
    r#"[{"Pay":{"who":"Triggering","cost":"{1}","unpaid":[{"Counter":{"what":"Triggering"}}]}}]"#
)]
#[case::by_a_spell(
    "Whenever you or a permanent you control becomes the target of a spell, draw a card.",
    &pair(r#"["Spell"]"#),
    r#"[{"Draw":{}}]"#
)]
#[case::amulet(
    "Whenever you become the target of a spell or ability an opponent controls, counter that spell or ability unless its controller pays {1}.",
    r#"{"PlayerBecomesTarget":{"who":"You","by":[{"ControlledBy":"Opponents"}]}}"#,
    r#"[{"Pay":{"who":"Triggering","cost":"{1}","unpaid":[{"Counter":{"what":"Triggering"}}]}}]"#
)]
#[case::gomazoa(
    "Whenever you become the target of a spell, you may untap ~.",
    r#"{"PlayerBecomesTarget":{"who":"You","by":["Spell"]}}"#,
    r#"[{"May":{"effects":[{"Untap":{"what":"This"}}]}}]"#
)]
#[case::gerbils(
    "Whenever you give a gift, draw a card.",
    r#"{"Did":{"action":"GaveGift","who":"You"}}"#,
    r#"[{"Draw":{}}]"#
)]
#[case::opponent_discovers(
    "Whenever an opponent discovers, you gain 2 life.",
    r#"{"Did":{"action":"Discovered","who":"Opponents"}}"#,
    r#"[{"GainLife":{"amount":2}}]"#
)]
#[case::you_investigate(
    "Whenever you investigate, draw a card.",
    r#"{"Did":{"action":"Investigated","who":"You"}}"#,
    r#"[{"Draw":{}}]"#
)]
#[case::player_manifests_dread(
    "Whenever a player manifests dread, you gain 1 life.",
    r#"{"Did":{"action":"ManifestedDread","who":"EachPlayer"}}"#,
    r#"[{"GainLife":{"amount":1}}]"#
)]
fn l31a_events_read_and_write_back(#[case] text: &str, #[case] when: &str, #[case] effects: &str) {
    let json = format!(r#"{{"Triggered":{{"when":{when},"effects":{effects}}}}}"#);
    reads_as(text, ClauseKind::Triggered, false, &json);
}

#[rstest]
#[case::another_player(
    "Whenever another player or a permanent you control becomes the target of a spell, draw a card."
)]
#[case::creature(
    "Whenever you or a creature you control becomes the target of a spell, draw a card."
)]
#[case::and_or(
    "Whenever you and/or at least one permanent you control becomes the target of a spell or ability an opponent controls, draw a card."
)]
#[case::gift_given("Whenever you give a card, draw a card.")]
#[case::manifest("Whenever you manifest, draw a card.")]
fn l31a_events_near_misses_read_otherwise(#[case] text: &str) {
    let read = clause(text, ClauseKind::Triggered, false);
    let ours = |when: &TriggerPattern| match when {
        TriggerPattern::PlayerBecomesTarget { .. } => true,
        TriggerPattern::AnyOf(found) => found
            .iter()
            .any(|one| matches!(one, TriggerPattern::PlayerBecomesTarget { .. })),
        TriggerPattern::Did { action, .. } => matches!(
            action,
            crate::did::Action::GaveGift | crate::did::Action::ManifestedDread
        ),
        _ => false,
    };
    assert!(
        !matches!(&read, Some(Ability::Triggered { when, .. }) if ours(when)),
        "{text}: {read:?}"
    );
}
