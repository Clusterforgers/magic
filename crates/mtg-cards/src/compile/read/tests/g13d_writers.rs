//! Goal: the round 13 writers give the oracle wording of an attack at you and your planeswalkers or at your opponents, and a trigger of another meaning (attacks at you alone, your own attackers, two or more attackers) never reads back as the clause.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn attacks(what: &str, at_least: u32) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"GroupAttacks":{{"what":{what},"at_least":{at_least}}}}},"effects":[{{"Draw":{{}}}}]}}}}"#
    )
}

const AT_YOU: &str =
    "Whenever an opponent attacks you and/or one or more planeswalkers you control, draw a card.";
const AT_YOU_IF: &str = "Whenever an opponent attacks, if they attacked you and/or a planeswalker you control, draw a card.";
const AT_THEM: &str = "Whenever one or more of your opponents are attacked, draw a card.";

#[rstest]
#[case::at_you_and_yours(
    attacks(r#"[{"Type":"Creature"},{"AttackingPlayerOrPlaneswalker":"You"}]"#, 1),
    AT_YOU_IF,
    true
)]
#[case::by_an_opponent(
    attacks(r#"[{"Type":"Creature"},{"ControlledBy":"Opponents"},{"AttackingPlayerOrPlaneswalker":"You"}]"#, 1),
    AT_YOU,
    true
)]
#[case::at_you_alone(
    attacks(r#"[{"Type":"Creature"},{"AttackingPlayer":"You"}]"#, 1),
    AT_YOU,
    false
)]
#[case::two_attackers(
    attacks(r#"[{"Type":"Creature"},{"AttackingPlayerOrPlaneswalker":"You"}]"#, 2),
    AT_YOU_IF,
    false
)]
#[case::at_opponents_and_theirs(
    attacks(
        r#"[{"Type":"Creature"},{"AttackingPlayerOrPlaneswalker":"Opponents"}]"#,
        1
    ),
    AT_YOU,
    false
)]
#[case::opponents_attacked(
    attacks(r#"[{"Type":"Creature"},{"AttackingPlayer":"Opponents"}]"#, 1),
    AT_THEM,
    true
)]
#[case::your_attackers(
    attacks(
        r#"[{"Type":"Creature"},{"ControlledBy":"You"},{"AttackingPlayer":"Opponents"}]"#,
        1
    ),
    AT_THEM,
    false
)]
#[case::opponents_and_their_planeswalkers(
    attacks(
        r#"[{"Type":"Creature"},{"AttackingPlayerOrPlaneswalker":"Opponents"}]"#,
        1
    ),
    AT_THEM,
    false
)]
fn g13d_attacks_at_some_players(#[case] json: String, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(&json, clause), reads, "{json}");
}
