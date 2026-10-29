//! Goal: a limit on the attacks at you under a condition, a limit on each opponent's blocks for one combat and the -4 of
//! The Eternal Wanderer read into their data and write back as the card's own text; the +1 of The Eternal Wanderer (an
//! exile returned at its owner's next end step) writes back; the near-misses stay with the table.
use super::*;

#[rstest]
#[case::tapped_at_you(
    "As long as ~ is tapped, no more than one creature can attack you each combat.",
    ClauseKind::Static,
    r#"{"Rule":{"rule":{"AttackLimitAt":{"most":1,"at":"You"}},"condition":{"Matches":{"subject":"This","selector":["Tapped"]}}}}"#
)]
#[case::untapped_block_limit(
    "As long as ~ is untapped, no more than two creatures can block each combat.",
    ClauseKind::Static,
    r#"{"Rule":{"rule":{"BlockLimit":2},"condition":{"Matches":{"subject":"This","selector":["Untapped"]}}}}"#
)]
#[case::each_opponent_blocks(
    "Whenever ~ attacks, each opponent can't block with more than one creature this combat.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Rule":{"rule":{"BlockLimitEach":{"most":1,"who":"Opponents"}},"duration":"UntilEndOfCombat"}}]}}"#
)]
#[case::defending_player_blocks(
    "Defending player can't block with more than two creatures this combat.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Rule":{"rule":{"BlockLimitEach":{"most":2,"who":"Defending"}},"duration":"UntilEndOfCombat"}}]}}"#
)]
#[case::keep_one_each(
    "For each player, choose a creature that player controls. Each player sacrifices all creatures they control not chosen this way.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[
        {"Choose":{"from":{"Each":[{"Type":"Creature"},{"ControlledBy":{"Bound":"player"}}]},"as":"kept","rest_as":"rest"}},
        {"Sacrifice":{"who":{"Bound":"player"},"filter":[{"Is":{"Bound":"rest"}}],"count":{"Count":[{"Is":{"Bound":"rest"}}]}}}]}}]}}"#
)]
fn w14a_the_limits_and_the_sacrifice_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::an_attack_limit_this_turn(
    "As long as ~ is tapped, no more than one creature can attack you each turn.",
    ClauseKind::Static
)]
#[case::no_comma(
    "As long as ~ is tapped no more than one creature can attack you each combat.",
    ClauseKind::Static
)]
#[case::zero_blockers(
    "Each opponent can't block with more than zero creatures this combat.",
    ClauseKind::Spell
)]
#[case::plural_of_one(
    "Each opponent can't block with more than one creatures this combat.",
    ClauseKind::Spell
)]
#[case::this_turn(
    "Each opponent can't block with more than one creature this turn.",
    ClauseKind::Spell
)]
#[case::you_block(
    "You can't block with more than one creature this combat.",
    ClauseKind::Spell
)]
#[case::opponent_chooses(
    "For each player, choose a creature that player controls. Each player sacrifices all creatures they control.",
    ClauseKind::Spell
)]
fn w14a_near_misses_stay_with_the_table(#[case] text: &str, #[case] kind: ClauseKind) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}

#[rstest]
#[case::owners_end_step(
    r#"{"OwnerOf":{"Bound":"exiled"}}"#,
    "Return that card to the battlefield under its owner's control at the beginning of that player's next end step."
)]
#[case::any_end_step(
    r#""EachPlayer""#,
    "Return that card to the battlefield under its owner's control at the beginning of the next end step."
)]
fn w14a_an_exile_returned_at_an_end_step_writes_back(#[case] whose: &str, #[case] second: &str) {
    let json = format!(
        r#"{{"Loyalty":{{"cost":{{"Plus":1}},"targets":[{{"what":{{"Object":[{{"AnyOf":[{{"Type":"Artifact"}},{{"Type":"Creature"}}]}}]}},"bounds":{{"min":0,"max":1}}}}],
        "effects":[{{"Exile":{{"what":{{"Target":0}},"as":"exiled"}}}},
        {{"DelayedTrigger":{{"when":{{"StepBegins":{{"step":"End","whose":{whose}}}}},"once":true,
        "effects":[{{"Move":{{"what":{{"Bound":"exiled"}},"to":{{"Battlefield":{{}}}}}}}}]}}}}]}}}}"#
    );
    let read = ability(&json);
    let wordings = render_variants(&read);
    let text = format!("+1: Exile up to one target artifact or creature. {second}");
    assert!(
        wordings.iter().any(|written| same_text(written, &text)),
        "{text}: {wordings:?}"
    );
}

#[test]
fn w14a_another_players_end_step_is_not_written() {
    let json = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],
        "effects":[{"Exile":{"what":{"Target":0},"as":"exiled"}},
        {"DelayedTrigger":{"when":{"StepBegins":{"step":"End","whose":"You"}},"once":true,
        "effects":[{"Move":{"what":{"Bound":"exiled"},"to":{"Battlefield":{}}}}]}}]}}"#;
    let wordings = render_variants(&ability(json));
    assert!(
        !wordings.iter().any(|written| written.contains("end step")),
        "{wordings:?}"
    );
}
