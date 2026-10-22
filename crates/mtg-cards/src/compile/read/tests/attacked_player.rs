//! Goal: the wordings of the attacks at an enchanted player ("whenever enchanted player is attacked", "whenever a player attacks enchanted player with one or more creatures", "each opponent attacking that player does the same") read into the shapes of the hand-written files, write back as the card's own text, and the near-misses stay with the table.
use super::*;

const OPPONENTS: &str = r#"{"ControllerOf":{"Each":[{"AttackingPlayer":"AttachedPlayer"},{"ControlledBy":"Opponents"}]}}"#;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::gain_life(
    "Whenever enchanted player is attacked, you gain 2 life. Each opponent attacking that player does the same.",
    r#"{"GainLife":{"amount":2}}"#,
    r#"{"GainLife":{"who":OPPONENTS,"amount":2}}"#
)]
#[case::draw(
    "Whenever enchanted player is attacked, you draw a card. Each opponent attacking that player does the same.",
    r#"{"Draw":{}}"#,
    r#"{"Draw":{"who":OPPONENTS}}"#
)]
#[case::a_named_token(
    "Whenever enchanted player is attacked, create a Gold token. Each opponent attacking that player does the same.",
    r#"{"CreateToken":{"token":{"Named":"gold"}}}"#,
    r#"{"CreateToken":{"who":OPPONENTS,"token":{"Named":"gold"}}}"#
)]
#[case::untap(
    "Whenever enchanted player is attacked, untap all nonland permanents you control. Each opponent attacking that player untaps all nonland permanents they control.",
    r#"{"Untap":{"what":{"Each":[{"Not":{"Type":"Land"}},{"ControlledBy":"You"}]}}}"#,
    r#"{"Untap":{"what":{"Each":[{"Not":{"Type":"Land"}},{"ControlledBy":OPPONENTS}]}}}"#
)]
fn the_attackers_do_the_same(#[case] text: &str, #[case] first: &str, #[case] second: &str) {
    let json = format!(
        r#"{{"Triggered":{{"when":{{"PlayerAttacked":{{"who":"AttachedPlayer"}}}},"effects":[{},{}]}}}}"#,
        first.replace("OPPONENTS", OPPONENTS),
        second.replace("OPPONENTS", OPPONENTS)
    );
    reads_as(text, ClauseKind::Triggered, false, &json);
}

#[rstest]
#[case::enchanted_player(
    "Whenever a player attacks enchanted player with one or more creatures, you gain 1 life.",
    r#"{"Triggered":{"when":{"PlayerAttacks":{"at":"AttachedPlayer"}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::you(
    "Whenever an opponent attacks you with one or more creatures, you draw a card.",
    r#"{"Triggered":{"when":{"PlayerAttacks":{"by":"Opponents","at":"You"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::a_player_attacked(
    "Whenever a player is attacked, you draw a card.",
    r#"{"Triggered":{"when":{"PlayerAttacked":{"who":"EachPlayer"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::you_are_attacked(
    "Whenever you are attacked, you gain 1 life.",
    r#"{"Triggered":{"when":{"PlayerAttacked":{"who":"You"}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
fn the_events_of_an_attack_read_as_triggers(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::another_player("Whenever target player is attacked, you gain 2 life.")]
#[case::with_a_creature("Whenever enchanted player is attacked with a creature, you gain 2 life.")]
#[case::without_the_creatures("Whenever a player attacks enchanted player, you gain 2 life.")]
#[case::the_same_for_the_attackers_of_another(
    "Whenever a player is attacked, you gain 2 life. Each opponent attacking that player does the same."
)]
#[case::the_same_thing(
    "Whenever enchanted player is attacked, you gain 2 life. Each opponent attacking that player does the same thing."
)]
#[case::an_effect_that_names_no_player(
    "Whenever enchanted player is attacked, scry 1. Each opponent attacking that player does the same."
)]
#[case::an_untap_that_says_the_same(
    "Whenever enchanted player is attacked, untap all nonland permanents you control. Each opponent attacking that player does the same."
)]
#[case::nothing_to_repeat(
    "Whenever enchanted player is attacked, each opponent attacking that player does the same."
)]
fn other_attacks_stay_with_the_table(#[case] text: &str) {
    assert_eq!(trigger(text), None);
}
