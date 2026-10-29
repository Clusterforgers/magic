//! The payment `w20c_counter` reads: "counter that spell or ability unless its controller pays {2}".

use super::Writer;
use crate::cost::{Cost, CostItem};
use crate::effect::Effect;
use crate::refs::Ref;

impl Writer<'_> {
    /// A payment: "counter that spell or ability unless its controller pays {2}", or the other wordings of one.
    pub(super) fn w20c_payment(
        &mut self,
        who: &Ref,
        cost: &Cost,
        paid: &[Effect],
        unpaid: &[Effect],
    ) -> Option<String> {
        Self::w20c_counter_unless(who, cost, paid, unpaid)
            .or_else(|| self.payment(who, cost, paid, unpaid))
    }

    /// "counter that spell or ability unless its controller pays {2}".
    fn w20c_counter_unless(
        who: &Ref,
        cost: &Cost,
        paid: &[Effect],
        unpaid: &[Effect],
    ) -> Option<String> {
        let Ref::ControllerOf(of) = who else {
            return None;
        };
        let [CostItem::Mana(mana)] = cost.0.as_slice() else {
            return None;
        };
        let [
            Effect::Counter {
                what: Ref::Triggering,
            },
        ] = unpaid
        else {
            return None;
        };
        (**of == Ref::Triggering && paid.is_empty()).then_some(())?;
        Some(format!(
            "counter that spell or ability unless its controller pays {mana}"
        ))
    }
}
