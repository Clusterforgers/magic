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
        "wasDealtCombatDamageThisTurn" => {
            some(&json!({"Tallied": {"who": "You", "what": "CombatDamageTaken"}}))
        }
        "wasDealtNonCombatDamageThisTurn" => {
            some(&json!({"Tallied": {"who": "You", "what": "NoncombatDamageTaken"}}))
        }
        // Corrupted: three or more poison counters (CR 702.181 reminder text).
        "IsCorrupted" => json!({"Compare": {
            "left": {"Counters": {"on": "You", "counter": "poison"}}, "op": "Ge", "right": 3}}),
        _ => return than_you(word),
    })
}

/// The permanents of a type `who` controls, counted.
fn controlled(kind: &str, who: &str) -> Value {
    json!({"Count": [{"Type": kind}, {"ControlledBy": who}]})
}

/// A comparison with the player who asks ("who controls more lands than you", "fewer creatures than you", "at least two
/// more lands than you"), written for the player tested as "you" and the asker as `Asker`; `None` for another word.
fn than_you(word: &str) -> Option<Value> {
    let rest = word.strip_suffix("ThanYou")?;
    let (op, more, rest) = if let Some(rest) = rest.strip_prefix("withMore") {
        ("Gt", 0, rest)
    } else if let Some(rest) = rest.strip_prefix("hasFewer") {
        ("Lt", 0, rest.strip_suffix("InPlay")?)
    } else {
        let rest = rest.strip_prefix("withAtLeast")?;
        let at = rest.find(|c: char| !c.is_ascii_digit())?;
        let more = rest[..at].parse::<u32>().ok().filter(|n| *n >= 1)?;
        let rest = rest[at..].strip_prefix("More")?;
        // "At least one more" is "more".
        if more == 1 {
            ("Gt", 0, rest)
        } else {
            ("Ge", more, rest)
        }
    };
    let kind = match rest {
        "Lands" => "Land",
        "Creatures" => "Creature",
        "Artifacts" => "Artifact",
        "Enchantments" => "Enchantment",
        _ => return None,
    };
    let theirs = controlled(kind, "Asker");
    let right = if more == 0 {
        theirs
    } else {
        json!({"Sum": [theirs, more]})
    };
    Some(json!({"Compare": {"left": controlled(kind, "You"), "op": op, "right": right}}))
}

/// `GE2 SpellsCastThisTurn`: the player's own count of something this turn compared with a number.
fn counted(ctx: &Ctx, word: &str) -> Option<Tr<Value>> {
    let (head, what) = word.split_once(' ')?;
    let at = head.find(|c: char| c.is_ascii_digit())?;
    let op = match &head[..at] {
        "GE" => "Ge",
        "GT" => "Gt",
        "LE" => "Le",
        "LT" => "Lt",
        "EQ" => "Eq",
        _ => return None,
    };
    let right = head[at..].parse::<u32>().ok()?;
    let tallied = |what: &str| Ok(json!({"Tallied": {"who": "You", "what": what}}));
    let left = match what {
        "SpellsCastThisTurn" => tallied("SpellsCast"),
        "CardsDiscardedThisTurn" => tallied("Discarded"),
        "CardsDrawn" => tallied("Drawn"),
        other if other.starts_with("ThisTurnEntered_") => {
            super::amount_more::entered_this_turn(ctx, other)
        }
        _ => return None,
    };
    Some(left.map(|left| json!({"Compare": {"left": left, "op": op, "right": right}})))
}

/// The cards in a player's hand, as an amount.
fn hand(who: &str) -> Value {
    json!({"Count": [{"In": "Hand"}, {"OwnedBy": who}]})
}

/// Hand-size properties: `HasCardsInHand_Card_GTY` (more cards in hand than the asker) and `_LE<n>` (at most n cards).
fn hand_size(word: &str) -> Option<Value> {
    let rest = word.strip_prefix("HasCardsInHand_Card_")?;
    if rest == "GTY" {
        return Some(json!({"Compare": {"left": hand("You"), "op": "Gt", "right": hand("Asker")}}));
    }
    let most = rest.strip_prefix("LE")?.parse::<u32>().ok()?;
    Some(json!({"Compare": {"left": hand("You"), "op": "Le", "right": most}}))
}

/// The condition a player property states, written for "you" (`HasCardsInGraveyard_Instant,Sorcery_GE1` counts the
/// cards of the types the player owns there); `None` for a word the table lacks.
pub fn property(ctx: &Ctx, word: &str) -> Option<Tr<Value>> {
    if let Some(found) = plain(word) {
        return Some(Ok(found));
    }
    if let Some(found) = hand_size(word) {
        return Some(Ok(found));
    }
    if let Some(rest) = word.strip_prefix("Condition") {
        return counted(ctx, rest);
    }
    if let Some(kind) = word.strip_prefix("controlsCreature.powerGE")
        && let Ok(least) = kind.parse::<u32>()
    {
        let spec = format!("Creature.powerGE{least}+YouCtrl");
        return Some(
            selector_in(ctx, &spec, None)
                .map(|of| json!({"Compare": {"left": {"Count": of}, "op": "Ge", "right": 1}})),
        );
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
