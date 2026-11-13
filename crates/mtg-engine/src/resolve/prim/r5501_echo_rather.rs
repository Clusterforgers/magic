//! Another cost for echo (Thick-Skinned Goblin, CR 702.30): while its controller has a rule `EchoRather`, the payment
//! of a permanent's echo cost offers that cost beside the echo cost, as a payment of one of several costs
//! (`r281_pay_one_of.rs`); paying either counts as paying echo.

use mtg_cards::cost::Cost;
use mtg_cards::did::Action;
use mtg_cards::effect::Effect;

use crate::characteristics::controller_of;
use crate::resolve::Cx;

/// The costs the payer of an echo payment may pay, the echo cost first; `None` when the effect is no echo payment or no
/// rule offers another cost to the controller of the permanent.
pub(super) fn r5501_alternatives(cx: &Cx<'_>, effect: &Effect) -> Option<Vec<Cost>> {
    let Effect::Pay { cost, paid, .. } = effect else {
        return None;
    };
    let echo = matches!(
        paid.as_slice(),
        [Effect::Performed {
            action: Action::PaidEcho,
            ..
        }]
    );
    if !echo {
        return None;
    }
    let state = cx.state();
    let controller = controller_of(state, cx.scope.source.id)?;
    let rather = crate::rules::echo_rather(state, controller);
    if rather.is_empty() {
        return None;
    }
    let mut costs = vec![cost.clone()];
    costs.extend(rather.into_iter().filter(|other| other != cost));
    (costs.len() > 1).then_some(costs)
}
