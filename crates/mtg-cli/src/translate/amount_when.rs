//! `Count$` expressions that pick between two amounts: `Count$Compare Y GE1.2.1` (2 when Y is at least 1, else 1) and
//! `Count$Kicked.2.0` (a named fact), as `Amount::When`.

use serde_json::{Value, json};

use super::amount::amount;
use super::condition::named;
use super::ctx::{Ctx, Tr, unmapped};
use super::valid::cmp;

/// The amount of a `Count$` base that is a choice between two amounts, `None` for any other base.
pub fn when(ctx: &Ctx, base: &str) -> Option<Tr<Value>> {
    if let Some(rest) = base.strip_prefix("Compare ") {
        return Some(compare(ctx, rest));
    }
    if let Some(rest) = base.strip_prefix("Adamant.") {
        return Some(adamant(ctx, rest));
    }
    let (word, branches) = base.split_once('.')?;
    let condition = match word {
        "Kicked" => json!("Kicked"),
        // The gift was promised: one when its caster promised it, else none.
        "PromisedGift" => json!({"Compare": {"left": "Promised", "op": "Ge", "right": 1}}),
        "Bargained" | "Teamwork" => json!("AdditionalCostPaid"),
        "Monarch" => json!({"Monarch": "You"}),
        "Morbid" => {
            json!({"Compare": {"left": {"Tallied": {"who": "EachPlayer", "what": "CreaturesDied"}}, "op": "Ge", "right": 1}})
        }
        "Blessing" => json!({"CitysBlessing": "You"}),
        "StartingPlayer" => json!({"StartingPlayer": "You"}),
        "Night" => json!("IsNight"),
        // Landfall: a land entered the battlefield under your control this turn.
        "Landfall" => {
            json!({"Compare": {"left": {"Tally": {"event": "Entered", "who": "You", "filter": [{"Type": "Land"}]}}, "op": "Ge", "right": 1}})
        }
        // An instant resolves in the phase it was cast in, so the phase now is the phase it was cast in.
        "IfCastInOwnMainPhase" => {
            json!({"All": ["YourTurn", {"Any": [{"Step": "Main1"}, {"Step": "Main2"}]}]})
        }
        // An {X} paid in the madness or foretell cost is not the X of the mana cost: only fixed amounts are read.
        "Madness" | "Foretold" if branches.contains('X') => {
            return Some(unmapped(format!("amount:Count${word}-with-X")));
        }
        "Madness" => json!({"CastWith": "Madness"}),
        "Foretold" => json!({"CastWith": "Foretold"}),
        // The three Urza lands together (Urza's Tower, Mine and Power-Plant, each one under your control).
        "UrzaLands" => {
            let held = |name: &str| json!({"Compare": {"left": {"Count": [{"Named": name}, {"ControlledBy": "You"}]}, "op": "Ge", "right": 1}});
            json!({"All": [held("Urza's Tower"), held("Urza's Mine"), held("Urza's Power Plant")]})
        }
        // How this card was cast (`wasCastFromGraveyard.2.1`): the cast-way atoms of a valid-string, asked of the source.
        _ if word.starts_with("wasCast") => {
            let selector = super::valid_atoms::atom(word)?;
            json!({"Matches": {"subject": "This", "selector": selector}})
        }
        "Threshold" | "Delirium" | "Hellbent" | "Metalcraft" | "Void" => match named(word) {
            Ok(condition) => condition,
            Err(gap) => return Some(Err(gap)),
        },
        _ => return None,
    };
    Some(branch(ctx, &condition, branches))
}

/// `White.1.0`: the amount when at least three mana of the color was spent to cast the spell (adamant), else the other.
fn adamant(ctx: &Ctx, rest: &str) -> Tr<Value> {
    let Some((color, branches)) = rest.split_once('.') else {
        return unmapped("amount:Count$Adamant-shape");
    };
    if !["White", "Blue", "Black", "Red", "Green"].contains(&color) {
        return unmapped("amount:Count$Adamant-color");
    }
    let spent = json!({"ManaSpent": {"of": color}});
    let condition = json!({"Compare": {"left": spent, "op": "Ge", "right": 3}});
    branch(ctx, &condition, branches)
}

/// `Y GE1.2.1`: an amount, a comparison with its threshold, the amount when it holds and the amount when it does not.
fn compare(ctx: &Ctx, rest: &str) -> Tr<Value> {
    let Some((left, test)) = rest.split_once(' ') else {
        return unmapped("amount:Count$Compare-shape");
    };
    let Some((op, threshold)) = test.get(..2).and_then(cmp).zip(test.get(2..)) else {
        return unmapped("amount:Count$Compare-operator");
    };
    let Some((threshold, branches)) = threshold.split_once('.') else {
        return unmapped("amount:Count$Compare-branches");
    };
    let condition = json!({"Compare": {
        "left": amount(ctx, left.trim())?, "op": op, "right": amount(ctx, threshold)?}});
    branch(ctx, &condition, branches)
}

/// `2.1`: the amount when the condition holds and the amount when it does not.
fn branch(ctx: &Ctx, condition: &Value, branches: &str) -> Tr<Value> {
    let Some((then, otherwise)) = branches.split_once('.') else {
        return unmapped("amount:Count-branches");
    };
    Ok(json!({"When": {
        "condition": condition, "then": amount(ctx, then)?, "otherwise": amount(ctx, otherwise)?}}))
}
