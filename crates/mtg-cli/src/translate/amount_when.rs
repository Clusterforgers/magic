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
    let (word, branches) = base.split_once('.')?;
    let condition = match word {
        "Kicked" => json!("Kicked"),
        "Monarch" => json!({"Monarch": "You"}),
        "Blessing" => json!({"CitysBlessing": "You"}),
        "StartingPlayer" => json!({"StartingPlayer": "You"}),
        "Night" => json!("IsNight"),
        "Threshold" | "Delirium" | "Hellbent" | "Metalcraft" => match named(word) {
            Ok(condition) => condition,
            Err(gap) => return Some(Err(gap)),
        },
        _ => return None,
    };
    Some(branch(ctx, &condition, branches))
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
