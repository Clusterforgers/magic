//! The spell or ability that made an object a target, countered unless its controller pays: "counter that spell or
//! ability unless its controller pays {2}".

use super::effects::mana_cost;
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::Amount;

/// "counter that spell or ability unless its controller pays {2}": the sentence of a trigger of a target. The payer is
/// `Triggering`, whose one player is the controller of the targeting spell or ability (as for ward); `ControllerOf` would
/// name the controller of the targeted object too.
pub(super) fn w20c_counter_unless(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.triggered.then_some(())?;
    r.need("counter that spell or ability unless its controller pays")?;
    let cost = mana_cost(r)?;
    Some(vec![Effect::Pay {
        who: Ref::Triggering,
        cost,
        times: Amount::one(),
        paid: Vec::new(),
        unpaid: vec![Effect::Counter {
            what: Ref::Triggering,
        }],
    }])
}
