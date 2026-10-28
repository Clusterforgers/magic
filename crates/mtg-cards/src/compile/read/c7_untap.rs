//! A tap that holds the permanents down through their controller's next untap step: "Tap target creature an opponent
//! controls. That creature doesn't untap during its controller's next untap step.", "Tap up to two target creatures.
//! They don't untap during their controller's next untap step." (CR 502.3)

use super::effects::object;
use super::tokens::Reader;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::Ref;

/// The ways a card names the one permanent the sentence before tapped: `(as the writer says it, as a card may)`.
const C7_UNTAP_ONE: [(&str, &str); 2] = [
    (
        ". It doesn't untap during its controller's next untap step",
        ". That creature doesn't untap during its controller's next untap step",
    ),
    (
        ". It doesn't untap during its controller's next untap step",
        ". That permanent doesn't untap during its controller's next untap step",
    ),
];

/// The tail of the sentence for one target and for several.
pub(super) const C7_UNTAP_TAILS: [&str; 2] = [
    "It doesn't untap during its controller's next untap step",
    "They don't untap during their controller's next untap step",
];

/// "tap <what>. <it> doesn't untap during <its controller's> next untap step": the tap and the rule that holds the
/// same permanents down, which lasts through that step.
pub(super) fn c7_untap_held(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("tap")?;
    let what = object(r)?;
    matches!(what, Ref::Target(0)).then_some(())?;
    r.punct('.').then_some(())?;
    if r.targets.first()?.bounds.is_one() {
        (r.word("it") || r.words("that creature") || r.words("that permanent")).then_some(())?;
        r.need("doesn't untap during its controller's next untap step")?;
    } else {
        r.need("they don't untap during their controller's next untap step")?;
    }
    Some(vec![
        Effect::Tap { what: what.clone() },
        Effect::Continuous {
            affected: what,
            mods: vec![Modification::DoesntUntap],
            duration: Duration::NextUntapStep,
        },
    ])
}

/// The card's words for the one permanent held down ("That creature doesn't untap ...") beside the writer's "It".
pub fn c7_untap_wordings(texts: &[String]) -> Vec<String> {
    C7_UNTAP_ONE
        .iter()
        .flat_map(|&(written, card)| {
            texts
                .iter()
                .filter(move |text| text.contains(written))
                .map(move |text| text.replacen(written, card, 1))
        })
        .collect()
}
