//! Players picked out by a property ("each opponent who lost life this turn", "players with seven or more cards in their
//! graveyard", "the players whose turn it isn't"): Forge's player properties read as a condition written for "you", one
//! table row per word, and the players they name as `Ref::PlayersWhere`.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr};
use super::valid::selector_in;

/// "At least one" of an amount.
fn some(left: &Value) -> Value {
    json!({"Compare": {"left": left, "op": "Ge", "right": 1}})
}

/// The condition a player property that needs no selector states, written for "you"; `None` for another word.
fn plain(word: &str) -> Option<Value> {
    Some(match word {
        "Active" => json!("YourTurn"),
        "NonActive" => json!({"Not": "YourTurn"}),
        "LostLifeThisTurn" => some(&json!({"Tallied": {"who": "You", "what": "LifeLost"}})),
        "wasDealtDamageThisTurn" => {
            some(&json!({"Tallied": {"who": "You", "what": "DamageTaken"}}))
        }
        // Corrupted: three or more poison counters (CR 702.181 reminder text).
        "IsCorrupted" => json!({"Compare": {
            "left": {"Counters": {"on": "You", "counter": "poison"}}, "op": "Ge", "right": 3}}),
        _ => return None,
    })
}

/// The condition a player property states, written for "you" (`HasCardsInGraveyard_Instant,Sorcery_GE1` counts the
/// cards of the types the player owns there); `None` for a word the table lacks.
pub fn property(ctx: &Ctx, word: &str) -> Option<Tr<Value>> {
    if let Some(found) = plain(word) {
        return Some(Ok(found));
    }
    let (zone, rest) = [
        ("Graveyard", "HasCardsInGraveyard_"),
        ("Hand", "HasCardsInHand_"),
    ]
    .into_iter()
    .find_map(|(zone, head)| word.strip_prefix(head).map(|rest| (zone, rest)))?;
    let (valid, least) = rest.rsplit_once("_GE")?;
    let least = least.parse::<u32>().ok().filter(|n| *n >= 1)?;
    let spec: Vec<String> = valid
        .split(',')
        .map(|kind| {
            let join = if kind.contains('.') { "+" } else { "." };
            format!("{kind}{join}YouOwn")
        })
        .collect();
    Some(
        selector_in(ctx, &spec.join(","), Some(zone))
            .map(|of| json!({"Compare": {"left": {"Count": of}, "op": "Ge", "right": least}})),
    )
}

/// The players of a group with the property: `who` is the group as a reference.
pub fn players(who: &Value, test: &Value) -> Value {
    json!({"PlayersWhere": {"who": who, "test": test}})
}

/// `Player.NonActive`, `Opponent.Active` and their kin: the players of the group with a property that needs no
/// selector; `None` for another word.
pub fn word(word: &str) -> Option<Value> {
    let (group, prop) = word.split_once('.')?;
    let who = match group {
        "Player" => json!("EachPlayer"),
        "Opponent" => json!("Opponents"),
        _ => return None,
    };
    plain(prop).map(|test| players(&who, &test))
}
