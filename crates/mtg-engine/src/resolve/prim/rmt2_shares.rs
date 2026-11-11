//! A prevention shield divided among targets ("prevent the next 5 damage that would be dealt this turn to any number of
//! targets, divided as you choose", CR 601.2d): one shield for each target with its share, fixed as it begins (CR 611.2c).

use mtg_cards::modify::Duration;
use mtg_cards::refs::{Ref, Selector};
use mtg_cards::trigger::ReplaceBy;
use mtg_core::ids::EffectId;

use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::rules::{Rule, RuleEffect};

/// The shields of a divided prevention, one for each recipient still legal with more than nothing as its share; none
/// when the amount reads no division (the caller makes one shield then).
pub(super) fn rmt2_divided_shields(
    cx: &Cx<'_>,
    (to, source, combat): (Option<&Ref>, &Selector, Option<bool>),
    by: &ReplaceBy,
    duration: &Duration,
) -> Result<Option<Run>, ResolveError> {
    let (Some(named), ReplaceBy::PreventCounted(amount)) = (to, by) else {
        return Ok(None);
    };
    if !crate::resolve::share::reads_division(amount) {
        return Ok(None);
    }
    let recipients = cx.scope.refs(named)?;
    let amounts = cx.scope.amounts_for(amount, &recipients)?;
    let state = cx.state();
    let mut events = Vec::new();
    for (recipient, share) in recipients.into_iter().zip(amounts) {
        let Ok(most) = u32::try_from(share) else {
            continue;
        };
        if most == 0 {
            continue;
        }
        let Some(mut shield) = super::shield::damage_shield(
            cx,
            (Some(named), source, combat),
            &ReplaceBy::PreventUpTo(most),
        )?
        else {
            continue;
        };
        shield.to = Some(vec![recipient]);
        let made = u64::try_from(events.len())
            .map_err(|_| ResolveError::Invalid("too many shields".into()))?;
        events.push(Event::RuleCreated {
            effect: Box::new(RuleEffect {
                id: EffectId(state.next_effect + made),
                rule: Rule::PreventDamage(Box::new(shield)),
                duration: duration.clone(),
                source: Some(cx.scope.source),
                controller: cx.controller(),
                turn: state.turn().number(),
            }),
        });
    }
    Ok(Some(Run::events(events)))
}
