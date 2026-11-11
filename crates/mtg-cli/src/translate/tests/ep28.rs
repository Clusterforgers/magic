//! Goal: the pieces of ep28 translate as the card prints them. A target player of a fact of the turn (`ValidTgts$ Player.LostLifeThisTurn`, `Player.attackedWithCreaturesThisTurn`, the same of `Opponent`) is a `PlayerAmong` of the players the fact holds for; another property stays a gap.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn guildmage(valid: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Human Shaman\nPT:2/2\nA:AB$ LoseLife | Cost$ B R | ValidTgts$ {valid} | LifeAmount$ 1 | SpellDescription$ Target player who lost life this turn loses 1 life.\n"
    )
}

#[rstest]
#[case::lost_life("Player.LostLifeThisTurn", "EachPlayer", "LifeLost")]
#[case::attacked("Player.attackedWithCreaturesThisTurn", "EachPlayer", "Attackers")]
#[case::an_opponent("Opponent.LostLifeThisTurn", "Opponents", "LifeLost")]
fn ep28_player_of_a_fact(#[case] valid: &str, #[case] who: &str, #[case] tally: &str) {
    let script = guildmage(valid);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let test = json!({"Compare": {"left": {"Tallied": {"who": "You", "what": tally}}, "op": "Ge", "right": 1}});
    let among = json!({"PlayerAmong": {"PlayersWhere": {"who": who, "test": test}}});
    assert_eq!(
        abilities(&script)[0]["Activated"]["targets"][0]["what"],
        among,
        "{valid}"
    );
}

#[rstest]
#[case::dealt_damage_by_the_source("Player.wasDealtDamageThisTurnBySource")]
#[case::another_fact("Player.gainedLifeThisTurn")]
#[case::not_the_owner("Player.!CardOwner")]
fn ep28_other_player_properties_stay_gaps(#[case] valid: &str) {
    assert!(!gaps(&guildmage(valid)).is_empty(), "{valid}");
}
