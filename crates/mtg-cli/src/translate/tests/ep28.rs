//! Goal: the pieces of ep28 translate as the card prints them. A target player of a fact of the turn (`ValidTgts$ Player.LostLifeThisTurn`, `Player.attackedWithCreaturesThisTurn`, the same of `Opponent`) is a `PlayerAmong` of the players the fact holds for; another property stays a gap. "~ enters under the control of an opponent of your choice" (`Moved | Layer$ Control`, `ChoosePlayer` of an opponent, `ChangeZone | GainControl$ ChosenPlayer`) is `UnderChosenOpponent`; any player, another card, no control or more steps stay gaps. Life lost last turn (`LifeLostLastTurn`) is `TalliedLastTurn`.
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

const UNDER: &str = "R:Event$ Moved | ValidCard$ Card.Self | Destination$ Battlefield | ReplaceWith$ DBChooseOpp | Layer$ Control | Description$ CARDNAME enters under the control of an opponent of your choice.\n";
const CHOOSE: &str = "SVar:DBChooseOpp:DB$ ChoosePlayer | Defined$ You | Choices$ Player.Opponent | ChoiceTitle$ Choose an opponent to give control to | AILogic$ Curse | SubAbility$ MoveToPlay\n";
const MOVE: &str = "SVar:MoveToPlay:DB$ ChangeZone | Hidden$ True | Origin$ All | Destination$ Battlefield | Defined$ ReplacedCard | GainControl$ ChosenPlayer | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearChosenPlayer$ True\n";

fn pendant(under: &str, choose: &str, moved: &str) -> String {
    format!("Name:T\nTypes:Artifact\n{under}{choose}{moved}")
}

#[test]
fn ep28_enters_under_a_chosen_opponent() {
    let script = pendant(UNDER, CHOOSE, MOVE);
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(
        abilities(&script),
        vec![
            json!({"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": "UnderChosenOpponent"}})
        ]
    );
}

#[rstest]
#[case::any_player(pendant(UNDER, &CHOOSE.replace("Player.Opponent", "Player"), MOVE))]
#[case::another_card(pendant(&UNDER.replace("Card.Self", "Creature"), CHOOSE, MOVE))]
#[case::no_control(pendant(UNDER, CHOOSE, &MOVE.replace("GainControl$ ChosenPlayer", "Tapped$ True")))]
#[case::more_after(pendant(UNDER, CHOOSE, &MOVE.replace("ClearChosenPlayer$ True", "ClearChosenPlayer$ True | SubAbility$ X\nSVar:X:DB$ Draw")))]
fn ep28_other_control_entries_stay_gaps(#[case] script: String) {
    assert!(!gaps(&script).is_empty(), "{script}");
}

fn upkeep(x: &str) -> String {
    format!(
        "Name:T\nTypes:Enchantment\nT:Mode$ Phase | Phase$ Upkeep | CheckSVar$ X | TriggerZones$ Battlefield | Execute$ TrigDraw | TriggerDescription$ At the beginning of each upkeep, if you lost life last turn, draw a card.\nSVar:TrigDraw:DB$ Draw | NumCards$ 1\nSVar:X:{x}\n"
    )
}

#[rstest]
#[case::you("PlayerCountPropertyYou$LifeLostLastTurn", "You")]
#[case::an_opponent("PlayerCountOpponents$LifeLostLastTurn", "Opponents")]
fn ep28_life_lost_last_turn(#[case] x: &str, #[case] who: &str) {
    let script = upkeep(x);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let found = serde_json::to_string(&abilities(&script)).unwrap_or_default();
    let wanted = format!(r#"{{"TalliedLastTurn":{{"what":"LifeLost","who":"{who}"}}}}"#);
    assert!(found.contains(&wanted), "{found}");
}
