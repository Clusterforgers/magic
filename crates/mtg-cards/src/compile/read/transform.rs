//! Transforming and flipping: "Transform ~.", "transform it" and "transform target creature you control" turn a
//! transforming double-faced permanent over (`Effect::Transform`); "Flip ~." and "flip it" flip a flip card
//! (`Effect::Flip`). The writer's side (`transform ~`, `flip ~`) is an arm of the renderer.

use super::effects::{is_on_the_battlefield, target_object};
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Targetable};

/// "Transform ~.", "transform it" (the card itself, named by the sentence before) and "transform target creature":
/// one permanent turns over; "flip ~" and "flip it" flip a flip card.
pub fn transform(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let flips = r.word("flip");
    if !flips {
        r.need("transform")?;
    }
    let what = if r.source() || (r.it == Some(Ref::This) && r.word("it")) {
        Ref::This
    } else {
        // A card in a graveyard, a hand or a library, or a spell, does not transform.
        let what = target_object(r)?;
        let spell = r.targets.first().is_some_and(|spec| {
            matches!(&spec.what, Targetable::Object(selector) if selector.0.contains(&Pred::Spell))
        });
        (!spell && is_on_the_battlefield(r, &what)).then_some(())?;
        r.it = Some(what.clone());
        what
    };
    Some(vec![if flips {
        Effect::Flip { what }
    } else {
        Effect::Transform { what }
    }])
}
