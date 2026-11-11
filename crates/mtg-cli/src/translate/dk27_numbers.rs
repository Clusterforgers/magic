//! `ChooseNumber` of every player with `Highest$` and `NotLowest$` (Wheel of Misfortune): each player chooses a number,
//! secretly with `Secretly$ True`; the players who chose the highest are dealt that much damage, and the players who did
//! not choose the lowest discard their hand and draw; and `BidLife` opened with any number (Pain's Reward). Any other
//! shape stays a gap.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, only, unmapped};
use crate::forge::script::Params;

/// The players who did not choose the lowest number.
fn not_lowest() -> Value {
    json!({"PlayersWhere": {"who": "EachPlayer", "test": {"Not": {"Among": {"Bound": "lowest"}}}}})
}

/// `Highest$`: damage equal to the highest number to the players who chose it, and nothing more.
fn highest(p: &Params) -> Tr<Value> {
    only(p, "DealDamage", &["DB", "Defined", "NumDmg"])?;
    match (p.get("DB"), p.get("Defined"), p.get("NumDmg")) {
        (Some("DealDamage"), Some("Remembered"), Some("Count$ChosenNumber")) => Ok(json!({
            "DealDamage": {"to": {"Bound": "highest"}, "amount": {"ChosenAs": "number"}}
        })),
        _ => unmapped("key:ChooseNumber.Highest"),
    }
}

/// `NotLowest$`: the players who did not choose the lowest discard their hand, then draw a fixed number of cards.
fn not_lowest_steps(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "Discard", &["DB", "Defined", "Mode", "SubAbility"])?;
    let discard = (p.get("DB"), p.get("Defined"), p.get("Mode"));
    if discard != (Some("Discard"), Some("Remembered"), Some("Hand")) {
        return unmapped("key:ChooseNumber.NotLowest");
    }
    let Some(then) = p.get("SubAbility") else {
        return unmapped("key:ChooseNumber.NotLowest");
    };
    let draw = ctx.sub(then)?;
    only(&draw, "Draw", &["DB", "Defined", "NumCards"])?;
    let count = draw.get("NumCards").and_then(|n| n.parse::<u32>().ok());
    let (Some("Draw"), Some("Remembered"), Some(count)) =
        (draw.get("DB"), draw.get("Defined"), count)
    else {
        return unmapped("key:ChooseNumber.NotLowest");
    };
    let hands = json!({"Count": [{"In": "Hand"}, {"OwnedBy": "EachPlayer"}]});
    Ok(vec![
        json!({"Discard": {"who": not_lowest(), "count": hands}}),
        json!({"Draw": {"who": not_lowest(), "count": count}}),
    ])
}

/// `BidLife` of every player opened with any number (`StartBidding$ Any`, Pain's Reward): the high bidder loses life
/// equal to the high bid (`LifeAmount$ X` with `X` the chosen number) and draws a fixed number of cards. A bid for a
/// target or against one player stays a gap.
pub fn dk27_bid_life(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "BidLife", &["StartBidding", "BidSubAbility"])?;
    if p.get("StartBidding") != Some("Any") {
        return unmapped("key:BidLife.StartBidding");
    }
    let Some(then) = p.get("BidSubAbility") else {
        return unmapped("key:BidLife.no-BidSubAbility");
    };
    let life_part = ctx.sub(then)?;
    only(
        &life_part,
        "LoseLife",
        &["DB", "Defined", "LifeAmount", "SubAbility"],
    )?;
    let shape = (
        life_part.get("DB"),
        life_part.get("Defined"),
        life_part.get("LifeAmount"),
    );
    let chosen = ctx.svar("X").map(str::trim) == Some("Count$ChosenNumber");
    if shape != (Some("LoseLife"), Some("Remembered"), Some("X")) || !chosen {
        return unmapped("key:BidLife.BidSubAbility");
    }
    let Some(then) = life_part.get("SubAbility") else {
        return unmapped("key:BidLife.BidSubAbility");
    };
    let draw = ctx.sub(then)?;
    only(&draw, "Draw", &["DB", "Defined", "NumCards"])?;
    let count = draw.get("NumCards").and_then(|n| n.parse::<u32>().ok());
    let (Some("Draw"), Some("Remembered"), Some(count)) =
        (draw.get("DB"), draw.get("Defined"), count)
    else {
        return unmapped("key:BidLife.BidSubAbility");
    };
    Ok(vec![
        json!({"BidLife": {"as": "bid", "winner": "bidder"}}),
        json!({"LoseLife": {"who": {"Bound": "bidder"}, "amount": {"ChosenAs": "bid"}}}),
        json!({"Draw": {"who": {"Bound": "bidder"}, "count": count}}),
    ])
}

/// Each player chooses a number 0 or greater (`Defined$ Player`), then the highest and the lowest are read.
pub fn dk27_secret_numbers(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "ChooseNumber",
        &["Defined", "Secretly", "Highest", "NotLowest"],
    )?;
    if p.get("Defined") != Some("Player") {
        return unmapped("key:ChooseNumber.Defined");
    }
    let secret = match p.get("Secretly") {
        Some("True") => true,
        None => false,
        Some(_) => return unmapped("key:ChooseNumber.Secretly"),
    };
    let (Some(high), Some(rest)) = (p.get("Highest"), p.get("NotLowest")) else {
        return unmapped("key:ChooseNumber.Highest");
    };
    let high = ctx.sub(high)?;
    let rest = ctx.sub(rest)?;
    let mut choose = json!({"as": "number", "highest": "highest", "lowest": "lowest"});
    if secret {
        choose["secret"] = json!(true);
    }
    let mut effects = vec![json!({ "ChooseNumbers": choose }), highest(&high)?];
    effects.extend(not_lowest_steps(ctx, &rest)?);
    Ok(effects)
}
