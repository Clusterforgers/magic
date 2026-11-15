//! A permanent's rule that changes a die's result after its controller rolls (r64-02, `GameRule::R6402ChangeRoll`):
//! "After you roll a die, you may pay 1 life. If you do, increase or decrease the result by 1. Do this only once each
//! turn." and "After you roll a die, you may remove a +1/+1 counter from ~. If you do, increase or decrease the result
//! by 1."

use super::tokens::Reader;
use crate::modify::{GameRule, R6402RollPay};
use crate::refs::Ref;

/// The words of the event and the offer.
const R6402_AFTER: &str = "after you roll a die";
/// The words of the change after "If you do,", before its amount.
const R6402_CHANGE: &str = "increase or decrease the result by";
/// The words of the limit.
const R6402_ONCE: &str = "do this only once each turn";

/// Reads the rule: the event, the cost, the change and its limit, each sentence with its period but the last.
pub(super) fn r6402_read(r: &mut Reader<'_>) -> Option<GameRule> {
    r.need(R6402_AFTER)?;
    r.punct(',').then_some(())?;
    r.need("you may")?;
    let pay = if r.word("pay") {
        let life = r.number()?;
        r.need("life")?;
        (life > 0).then_some(R6402RollPay::Life(life))?
    } else {
        r.need("remove a")?;
        let counter = super::counters::kind(r)?;
        r.need("counter from")?;
        r.source().then_some(())?;
        R6402RollPay::RemoveCounter(counter)
    };
    r.punct('.').then_some(())?;
    r.need("if you do")?;
    r.punct(',').then_some(())?;
    r.need(R6402_CHANGE)?;
    let by = r.number()?;
    (by > 0).then_some(())?;
    let once = r
        .attempt(|r| {
            r.punct('.').then_some(())?;
            r.need(R6402_ONCE)
        })
        .is_some();
    Some(GameRule::R6402ChangeRoll {
        who: Ref::You,
        pay,
        by,
        once,
    })
}

/// The words of a rule `r6402_read` reads, none for another rule.
pub(super) fn r6402_texts(rule: &GameRule) -> Vec<String> {
    let GameRule::R6402ChangeRoll {
        who: Ref::You,
        pay,
        by,
        once,
    } = rule
    else {
        return Vec::new();
    };
    if *by == 0 {
        return Vec::new();
    }
    let cost = match pay {
        R6402RollPay::Life(0) => return Vec::new(),
        R6402RollPay::Life(life) => format!("pay {life} life"),
        R6402RollPay::RemoveCounter(counter) => {
            let Some(kind) = super::counters::kind_text(counter) else {
                return Vec::new();
            };
            format!("remove a {kind} counter from ~")
        }
    };
    let mut text = format!(
        "After you roll a die, you may {cost}. If you do, increase or decrease the result by {by}"
    );
    if *once {
        text.push_str(". Do this only once each turn");
    }
    vec![text]
}
