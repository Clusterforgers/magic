//! Goal: any target with a fact of the turn (`ValidTgts$ Any.wasDealtDamageThisTurn`) translates as the creatures,
//! planeswalkers and battles with the property or a player the same fact holds for, never as a permanent alone; a
//! property no player can have stays a gap, and a target player dealt damage this turn is a `PlayerAmong`.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn needle(valid: &str) -> String {
    format!(
        "Name:T\nManaCost:R\nTypes:Instant\nA:SP$ DealDamage | ValidTgts$ {valid} | NumDmg$ 1 | SpellDescription$ CARDNAME deals 1 damage to any target that was dealt damage this turn.\n"
    )
}

#[test]
fn dk26_any_target_dealt_damage_this_turn() {
    let script = needle("Any.wasDealtDamageThisTurn");
    assert_eq!(gaps(&script), Vec::<String>::new());
    let what = &abilities(&script)[0]["Spell"]["targets"][0]["what"];
    let kinds = what["Either"].as_array().expect("either");
    assert_eq!(kinds.len(), 2, "{what}");
    let objects = kinds[0]["Object"].to_string();
    for word in ["Creature", "Planeswalker", "Battle", "DamagedThisTurn"] {
        assert!(objects.contains(word), "{word} in {objects}");
    }
    let test = json!({"Compare": {"left": {"Tallied": {"who": "You", "what": "DamageTaken"}}, "op": "Ge", "right": 1}});
    assert_eq!(
        kinds[1],
        json!({"PlayerAmong": {"PlayersWhere": {"who": "EachPlayer", "test": test}}})
    );
}

#[rstest]
#[case::a_permanent_property("Any.tapped")]
#[case::by_the_source("Any.wasDealtDamageThisTurnBySource")]
fn dk26_any_target_of_a_property_no_player_has_stays_a_gap(#[case] valid: &str) {
    assert!(!gaps(&needle(valid)).is_empty(), "{valid}");
}

#[test]
fn dk26_target_player_dealt_damage_this_turn() {
    let script = needle("Player.wasDealtDamageThisTurn");
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert!(
        abilities(&script)[0]["Spell"]["targets"][0]["what"]
            .get("PlayerAmong")
            .is_some()
    );
}
