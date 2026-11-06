//! More `Count$` bases and player-count words: the amounts that name a fact of the game (domain, party size, colors of
//! mana spent, the starting life), the permanents that entered or died this turn, and a player's cards and life.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::valid::selector_in;

/// The amount of a `Count$` base that is one of these, `None` for any other base.
pub fn count(ctx: &Ctx, base: &str) -> Option<Tr<Value>> {
    let (word, rest) = base.split_once(' ').unwrap_or((base, ""));
    Some(match word {
        "Domain" if rest.is_empty() => Ok(json!({"Distinct": {
            "of": [{"Type": "Land"}, {"ControlledBy": "You"}], "stat": "BasicLandTypes"}})),
        "ResolvedThisTurn" if rest.is_empty() => Ok(json!("ResolvedThisTurn")),
        // The permanents this one sacrificed as it entered (devour); another filter would count other cards.
        "Devoured" if rest == "Card" => Ok(json!("Devoured")),
        // The number an earlier step of the ability remembered; the size of a number is the number.
        "RememberedNumber" | "RememberedSize" if rest.is_empty() => {
            return super::number::read(ctx).map(Ok);
        }
        "Party" if rest.is_empty() => Ok(json!({"Party": "You"})),
        "CountersAddedThisTurn" => counters_added(ctx, rest),
        "CreaturesAttackedThisTurn" if !rest.is_empty() => attacked_this_turn(ctx, rest),
        "Converge" if rest.is_empty() => Ok(json!("ColorsSpent")),
        "CastTotalManaSpent" if rest.is_empty() => Ok(json!({"ManaSpent": {"of": null}})),
        "YourStartingLife" if rest.is_empty() => Ok(json!({"StartingLife": "You"})),
        // The sum over the opponents: all their poison counters together.
        "TotalOppPoisonCounters" if rest.is_empty() => {
            Ok(json!({"Counters": {"on": "Opponents", "counter": "poison"}}))
        }
        "CardsInYourHand" if rest.is_empty() => {
            Ok(json!({"Count": [{"In": "Hand"}, {"OwnedBy": "You"}]}))
        }
        // The mana value of the top card of your library; zero when it is empty.
        "TopOfLibraryCMC" if rest.is_empty() => Ok(json!({"Total": {
            "of": [{"In": "Library"}, {"OwnedBy": "You"}, "TopOfLibrary"], "stat": "ManaValue"}})),
        "TotalCommanderCastFromCommandZone" if rest.is_empty() => {
            Ok(json!({"CommanderCasts": "You"}))
        }
        // All the mana in your pool; the mana of one color is a different count and stays a gap.
        "ManaPool:All" if rest.is_empty() => Ok(json!({"ManaInPool": "You"})),
        "YouDescendedThisTurn" if rest.is_empty() => {
            Ok(json!({"Tallied": {"who": "You", "what": "Descended"}}))
        }
        // The cards that left your graveyard this turn; another filter would count other cards.
        "LeftGraveyardThisTurn" if rest == "Card.YouOwn" => {
            Ok(json!({"Tallied": {"who": "You", "what": "CardsLeftGraveyard"}}))
        }
        // The mana symbols of the color among the permanents you control: the devotion to the color.
        _ if word.starts_with("Chroma.") && rest.is_empty() => {
            devotion(&word.replacen("Chroma.", "Devotion.", 1))
        }
        _ if word.starts_with("YourCounters") && rest.is_empty() => your_counters(word),
        "Valid" | "ValidGraveyard" | "ValidHand" | "ValidExile" | "ValidLibrary"
        | "ValidBattlefield" => valid_count(ctx, word, rest),
        _ if word.starts_with("ThisTurnEntered_") => entered_this_turn(
            ctx,
            &if rest.is_empty() {
                word.to_owned()
            } else {
                format!("{word} {rest}")
            },
        ),
        _ if word.starts_with("Devotion.") || word.starts_with("DevotionDual.") => devotion(word),
        _ => return None,
    })
}

/// `CountersAddedThisTurn Any You Creature`: the counters of any kind that the players put this turn on the permanents the
/// valid-string selects. A kind of counter is a different count and stays a gap.
fn counters_added(ctx: &Ctx, rest: &str) -> Tr<Value> {
    let words: Vec<&str> = rest.split(' ').collect();
    let [kind, who, valid] = words[..] else {
        return unmapped("amount:Count$CountersAddedThisTurn");
    };
    let who = match who {
        "You" => "You",
        "Player" => "EachPlayer",
        "Opponent" => "Opponents",
        _ => return unmapped("amount:Count$CountersAddedThisTurn"),
    };
    if kind != "Any" {
        return unmapped("amount:Count$CountersAddedThisTurn-of-a-kind");
    }
    let filter = selector_in(ctx, valid, None)?;
    Ok(json!({"Tally": {"event": "CountersPut", "who": who, "filter": filter}}))
}

/// `CreaturesAttackedThisTurn Creature.token`: the creatures the valid-string selects that were declared as attackers this
/// turn, whoever controls them (the string names the controller itself); the effect's source is the object itself.
fn attacked_this_turn(ctx: &Ctx, valid: &str) -> Tr<Value> {
    let filter = selector_in(ctx, &valid.replace(".EffectSource", ".Self"), None)?;
    Ok(json!({"Tally": {"event": "Attacked", "who": "EachPlayer", "filter": filter}}))
}

/// `YourCountersEnergy`: the counters of that kind on you; a kind that is not one plain word stays a gap.
fn your_counters(word: &str) -> Tr<Value> {
    let kind = word.strip_prefix("YourCounters").unwrap_or_default();
    let known = ["Energy", "Experience", "Poison", "Rad", "Charge"];
    if !known.contains(&kind) {
        return unmapped(format!("amount:Count${word}"));
    }
    let Some(counter) = super::amount::counter_kind(&kind.to_uppercase()) else {
        return unmapped(format!("amount:Count${word}"));
    };
    Ok(json!({"Counters": {"on": "You", "counter": counter}}))
}

/// `Count$Valid Creature.YouCtrl$GreatestCardPower`: the objects the valid-string selects, counted or reduced to one
/// characteristic by the word after the `$`.
fn valid_count(ctx: &Ctx, word: &str, rest: &str) -> Tr<Value> {
    let zone = match word {
        "ValidGraveyard" => Some("Graveyard"),
        "ValidHand" => Some("Hand"),
        "ValidExile" => Some("Exile"),
        "ValidLibrary" => Some("Library"),
        "ValidBattlefield" => Some("Battlefield"),
        _ => None,
    };
    let (spec, reduce) = rest.split_once('$').unwrap_or((rest, ""));
    if spec.contains("SharesColorWith MostProminentColor") && !spec.contains("EnchantedBy") {
        return unmapped("amount:Count$Valid-most-common-color-of-a-group");
    }
    let of = selector_in(ctx, spec, zone)?;
    reduced(word, &of, reduce)
}

/// The objects the selector names, counted or reduced to one characteristic by the word after the `$` of the expression.
pub(super) fn reduced(word: &str, of: &[Value], reduce: &str) -> Tr<Value> {
    // All the counters of one kind (or of every kind) on the objects together.
    if let Some(kind) = reduce.strip_prefix("CardCounters.") {
        let on = json!({"Each": of});
        if kind == "ALL" {
            return Ok(json!({"AnyCounters": on}));
        }
        return match super::amount::counter_kind(&kind.to_uppercase()) {
            Some(counter) => Ok(json!({"Counters": {"on": on, "counter": counter}})),
            None => unmapped(format!("amount:Count${word}$CardCounters.{kind}")),
        };
    }
    let (kind, stat) = match reduce {
        "" => return Ok(json!({"Count": of})),
        "CardTypes" => return Ok(json!({"CardTypes": of})),
        "Colors" => ("Distinct", "Colors"),
        "CardPower" => ("Total", "Power"),
        "CardToughness" => ("Total", "Toughness"),
        "CardManaCost" => ("Total", "ManaValue"),
        "GreatestCardPower" => ("Greatest", "Power"),
        "GreatestCardToughness" => ("Greatest", "Toughness"),
        "GreatestCardManaCost" => ("Greatest", "ManaValue"),
        "DifferentCardPower" => ("Distinct", "Power"),
        "DifferentCardToughness" => ("Distinct", "Toughness"),
        "DifferentCardManaCost" => ("Distinct", "ManaValue"),
        "DifferentCardNames" => ("Distinct", "Name"),
        other => return unmapped(format!("amount:Count${word}${other}")),
    };
    Ok(json!({kind: {"of": of, "stat": stat}}))
}

/// `ThisTurnEntered_Battlefield_Creature.YouCtrl` and `ThisTurnEntered_Graveyard_from_Battlefield_Creature`: the
/// permanents that entered the battlefield, or went from it to a graveyard, this turn, as they were then.
fn entered_this_turn(ctx: &Ctx, word: &str) -> Tr<Value> {
    let spec = word.strip_prefix("ThisTurnEntered_").unwrap_or_default();
    let (event, valid) = if let Some(valid) = spec.strip_prefix("Battlefield_") {
        ("Entered", valid)
    } else if let Some(valid) = spec.strip_prefix("Graveyard_from_Battlefield_") {
        ("Died", valid)
    } else {
        return unmapped(format!("amount:Count$ThisTurnEntered_{}", head(spec)));
    };
    let filter = selector_in(ctx, valid, None)?;
    Ok(json!({"Tally": {"event": event, "who": "EachPlayer", "filter": filter}}))
}

/// `Devotion.Black` and `DevotionDual.White.Black`: the mana symbols of the colors among the permanents you control.
fn devotion(word: &str) -> Tr<Value> {
    let colors: Vec<&str> = word.split('.').skip(1).collect();
    let known = ["White", "Blue", "Black", "Red", "Green"];
    if colors.is_empty() || !colors.iter().all(|color| known.contains(color)) {
        return unmapped("amount:Count$Devotion");
    }
    Ok(json!({"Devotion": colors}))
}

fn head(spec: &str) -> &str {
    spec.split('.').next().unwrap_or_default()
}

/// A number about a player (`TargetedPlayer$CardsInHand`, `PlayerCountPropertyYou$CardsDiscardedThisTurn`).
pub fn player_amount(head: &str, what: &str, who: &Value) -> Option<Tr<Value>> {
    let cards = |zone: &str| Ok(json!({"Count": [{"In": zone}, {"OwnedBy": who}]}));
    let tallied = |what: &str| Ok(json!({"Tallied": {"who": who, "what": what}}));
    Some(match (head, what) {
        ("TargetedPlayer", "CardsInHand") => cards("Hand"),
        ("TargetedPlayer", "CardsInLibrary") => cards("Library"),
        ("TargetedPlayer", "CardsInGraveyard") => cards("Graveyard"),
        ("TargetedPlayer", "LifeTotal") => Ok(json!({"Life": who})),
        ("PlayerCountPropertyYou", "CardsDiscardedThisTurn") => tallied("Discarded"),
        ("PlayerCountPropertyYou", "LifeLostThisTurn") => tallied("LifeLost"),
        ("PlayerCountPropertyYou", "LandsPlayed") => tallied("LandsPlayed"),
        ("PlayerCountPropertyYou", "SacrificedThisTurn" | "SacrificedThisTurn Permanent") => {
            tallied("Sacrificed")
        }
        // The opponents the player's creatures attacked in this combat, directly or through a planeswalker.
        ("PlayerCountPropertyYou", "OpponentsAttackedThisCombat") => {
            Ok(json!({"Players": "Attacked"}))
        }
        // The creatures that attacked the player in this combat: one or more means the player has been attacked.
        ("PlayerCountPropertyYou", "HasPropertyBeenAttackedThisCombat") => Ok(json!({"Tally": {
            "event": "Attacked",
            "who": "EachPlayer",
            "filter": [{"DidThisTurn": {"event": "Attacked", "to": who, "combat": true}}],
        }})),
        _ => return None,
    })
}

/// `DungeonsCompleted$Amount`: the dungeons you have completed this game.
pub fn dungeons(what: &str) -> Tr<Value> {
    match what {
        "Amount" => Ok(json!({"Tallied": {"who": "You", "what": "DungeonsCompleted"}})),
        other => unmapped(format!("amount:DungeonsCompleted${other}")),
    }
}

/// A number about the player `who` of an event or a target (`LifeTotal`, `CardsInHand`, `Counters.Poison`); the gap names `head`.
pub fn player_stat(head: &str, what: &str, who: &Value) -> Tr<Value> {
    if let Some(found) = player_amount("TargetedPlayer", what, who) {
        return found;
    }
    match what.strip_prefix("Counters.") {
        Some(kind) => match super::amount::counter_kind(&kind.to_uppercase()) {
            Some(counter) => Ok(json!({"Counters": {"on": who, "counter": counter}})),
            None => unmapped(format!("amount:{head}$Counters.{kind}")),
        },
        None => unmapped(format!("amount:{head}${what}")),
    }
}

/// `PlayerCountOpponents$HighestCardsInHand` and its kin: the largest count among the players of the cards their own
/// selector names, each player taken as "you"; `None` for any other word.
pub fn highest(ctx: &Ctx, who: &str, what: &str) -> Option<Tr<Value>> {
    let mine = |zone: &str| Ok(vec![json!({"In": zone}), json!({"OwnedBy": "You"})]);
    // The most of one kind any single player holds: a maximum over the players, never their sum.
    if let Some(kind) = what.strip_prefix("HighestCounters.") {
        let Some(counter) = super::amount::counter_kind(&kind.to_uppercase()) else {
            return Some(unmapped(format!("amount:HighestCounters.{kind}")));
        };
        return Some(Ok(
            json!({"HighestCounters": {"who": who, "counter": counter}}),
        ));
    }
    let of = if let Some(spec) = what.strip_prefix("HighestValid ") {
        selector_in(ctx, spec, None)
    } else if let Some(spec) = what.strip_prefix("HighestValidGraveyard ") {
        selector_in(ctx, spec, Some("Graveyard"))
    } else {
        match what {
            "HighestCardsInHand" => mine("Hand"),
            "HighestCardsInGraveyard" => mine("Graveyard"),
            _ => return None,
        }
    };
    Some(of.and_then(|of| {
        // The selector is written for "you": a mention of the opponents would read the wrong players.
        if of.iter().any(|pred| pred.to_string().contains("Opponents")) {
            return unmapped("amount:Highest-of-the-opponents");
        }
        Ok(json!({"Largest": {"who": who, "of": of}}))
    }))
}

/// `PlayerCountRemembered$LifeTotal`: a number about the player the loop is on (`RepeatEach` binds it as the remembered
/// object), read for that player alone.
pub fn remembered_player(ctx: &Ctx, what: &str) -> Tr<Value> {
    let Some(who) = super::memory::slot_ref(ctx, super::memory::Slot::Remembered) else {
        return unmapped("amount:PlayerCountRemembered-without-a-player");
    };
    if let Some(spec) = what.strip_prefix("Valid ") {
        let of = selector_in(ctx, spec, None)?;
        if of.iter().any(|pred| pred.to_string().contains("Opponents")) {
            return unmapped("amount:PlayerCountRemembered-of-the-opponents");
        }
        return Ok(json!({"Largest": {"who": who, "of": of}}));
    }
    let head = match what {
        "LifeTotal" | "CardsInHand" | "CardsInLibrary" | "CardsInGraveyard" => "TargetedPlayer",
        _ => "PlayerCountPropertyYou",
    };
    match player_amount(head, what, &who) {
        Some(found) => found,
        None => unmapped(format!("amount:PlayerCountRemembered${what}")),
    }
}
