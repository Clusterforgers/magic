//! Goal: the writers of ep28 write the card's words and nothing near them. A target player a fact holds for ("target opponent who lost life this turn", `Targetable::PlayerAmong` of a `PlayersWhere`) writes back with the fact; another fact, other players or a plain player does not. A fact of last turn ("if you lost life last turn", `TalliedLastTurn`) writes the fact of this turn with "last turn"; this turn, another player, amount or count does not. "~ enters under the control of an opponent of your choice" reads and writes back; another permanent or a condition does not.
use super::*;

fn among(who: &str, test: &str, effect: &str) -> Ability {
    ability(&format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"PlayerAmong":{{"PlayersWhere":{{"who":"{who}","test":{test}}}}}}}}}],"effects":[{effect}]}}}}"#
    ))
}

const LOST: &str =
    r#"{"Compare":{"left":{"Tallied":{"who":"You","what":"LifeLost"}},"op":"Ge","right":1}}"#;
const ATTACKED: &str =
    r#"{"Compare":{"left":{"Tallied":{"who":"You","what":"Attackers"}},"op":"Ge","right":1}}"#;
const GAINED: &str =
    r#"{"Compare":{"left":{"Tallied":{"who":"You","what":"LifeGained"}},"op":"Ge","right":1}}"#;
const MORE_CREATURES: &str = r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Gt","right":{"Count":[{"Type":"Creature"},{"ControlledBy":"Asker"}]}}}"#;
const LOSE: &str = r#"{"LoseLife":{"who":{"Target":0},"amount":1}}"#;
const DRAW: &str = r#"{"Draw":{"who":{"Target":0},"count":2}}"#;

#[rstest]
#[case::lost_life(
    among("EachPlayer", LOST, LOSE),
    "Target player who lost life this turn loses 1 life.",
    true
)]
#[case::attacked(
    among("EachPlayer", ATTACKED, DRAW),
    "Target player who attacked this turn draws two cards.",
    true
)]
#[case::more_creatures(
    among("Opponents", MORE_CREATURES, DRAW),
    "Target opponent who controls more creatures than you draws two cards.",
    true
)]
#[case::an_opponent_is_no_player(
    among("Opponents", LOST, LOSE),
    "Target player who lost life this turn loses 1 life.",
    false
)]
#[case::another_fact(
    among("EachPlayer", ATTACKED, LOSE),
    "Target player who lost life this turn loses 1 life.",
    false
)]
#[case::no_writer_for_the_fact(
    among("EachPlayer", GAINED, LOSE),
    "Target player who gained life this turn loses 1 life.",
    false
)]
#[case::a_plain_player_is_not_one_of_a_fact(
    ability(r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":1}}]}}"#),
    "Target player who lost life this turn loses 1 life.",
    false
)]
#[case::the_fact_is_not_dropped(
    among("EachPlayer", LOST, LOSE),
    "Target player loses 1 life.",
    false
)]
fn ep28_target_player_of_a_fact(#[case] ability: Ability, #[case] text: &str, #[case] reads: bool) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

fn upkeep_if(left: &str, right: i32) -> Ability {
    ability(&format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep"}}}},"if":{{"Compare":{{"left":{left},"op":"Ge","right":{right}}}}},"effects":[{{"Draw":{{}}}}]}}}}"#
    ))
}

const YOU_LAST: &str = r#"{"TalliedLastTurn":{"who":"You","what":"LifeLost"}}"#;
const OPPONENT_LAST: &str = r#"{"TalliedLastTurn":{"who":"Opponents","what":"LifeLost"}}"#;
const RESPONSE: &str = "At the beginning of each upkeep, if you lost life last turn, draw a card.";
const FEAST: &str =
    "At the beginning of each upkeep, if an opponent lost life last turn, draw a card.";

#[rstest]
#[case::you(upkeep_if(YOU_LAST, 1), RESPONSE, true)]
#[case::an_opponent(upkeep_if(OPPONENT_LAST, 1), FEAST, true)]
#[case::this_turn_is_not_last_turn(
    upkeep_if(r#"{"Tallied":{"who":"You","what":"LifeLost"}}"#, 1),
    RESPONSE,
    false
)]
#[case::last_turn_is_not_this_turn(
    upkeep_if(YOU_LAST, 1),
    "At the beginning of each upkeep, if you lost life this turn, draw a card.",
    false
)]
#[case::another_player(upkeep_if(OPPONENT_LAST, 1), RESPONSE, false)]
#[case::another_amount(upkeep_if(YOU_LAST, 3), RESPONSE, false)]
#[case::another_count(
    upkeep_if(r#"{"TalliedLastTurn":{"who":"You","what":"LifeGained"}}"#, 1),
    RESPONSE,
    false
)]
fn ep28_a_fact_of_last_turn(#[case] ability: Ability, #[case] text: &str, #[case] reads: bool) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

const UNDER: &str = "~ enters under the control of an opponent of your choice.";

#[test]
fn ep28_entering_under_an_opponent_reads_and_writes() {
    let read = golden(UNDER, ClauseKind::Static, false);
    assert_eq!(
        read,
        ability(
            r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":"UnderChosenOpponent"}}"#
        )
    );
}

#[rstest]
#[case::under_an_opponent(r#"{"Enters":{"what":["This"]}}"#, None, true)]
#[case::another_permanent(r#"{"Enters":{"what":[{"Type":"Creature"}]}}"#, None, false)]
#[case::under_a_condition(r#"{"Enters":{"what":["This"]}}"#, Some(r#""YourTurn""#), false)]
fn ep28_entering_under_an_opponent_near_misses_differ(
    #[case] when: &str,
    #[case] condition: Option<&str>,
    #[case] reads: bool,
) {
    let condition = condition.map_or(String::new(), |c| format!(r#","condition":{c}"#));
    let found = written_variants(&ability(&format!(
        r#"{{"Replacement":{{"when":{when},"by":"UnderChosenOpponent"{condition}}}}}"#
    )));
    assert_eq!(
        found.iter().any(|w| same_text(w, UNDER)),
        reads,
        "{found:?}"
    );
}
