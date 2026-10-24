//! A token made by populate and what happens to it: "Populate. The token created this way gains haste. Sacrifice it at the
//! beginning of the next end step."

use super::{Effect, Ref, TOKENS, copies};
use crate::compile::read::selectors::granted_text;
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Selector};
use crate::trigger::{Step, TriggerPattern};
use crate::value::Amount;

/// "populate", the keywords the token gains for good and its sacrifice at the next end step; the number of effects it
/// takes. The token is bound so that the steps after populate can name it.
pub(super) fn text(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        choose,
        Effect::CreateToken { bind, .. },
        Effect::Continuous {
            affected,
            mods,
            duration: Duration::Forever,
        },
        Effect::DelayedTrigger {
            when:
                TriggerPattern::StepBegins {
                    step: Step::End,
                    whose: Ref::EachPlayer,
                },
            once: true,
            effects: later,
        },
        ..,
    ] = effects
    else {
        return None;
    };
    let token = Ref::Bound(TOKENS.to_owned());
    (bind.as_deref() == Some(TOKENS) && *affected == token).then_some(())?;
    // Populate without the name of its token is the two steps the readers know.
    let mut steps = vec![choose.clone(), effects.get(1)?.clone()];
    if let Some(Effect::CreateToken { bind, .. }) = steps.get_mut(1) {
        *bind = None;
    }
    copies::is_populate(&steps).then_some(())?;
    let [
        Effect::Sacrifice {
            who: Ref::You,
            filter,
            count,
        },
    ] = later.as_slice()
    else {
        return None;
    };
    (*filter == Selector(vec![Pred::Is(token)]) && *count == Amount::one()).then_some(())?;
    (mods
        .iter()
        .all(|given| matches!(given, Modification::AddKeyword(_))))
    .then_some(())?;
    Some((
        format!(
            "populate. The token created this way gains {}. Sacrifice it at the beginning of the next end step",
            granted_text(mods)?
        ),
        4,
    ))
}
