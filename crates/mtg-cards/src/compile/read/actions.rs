//! Keyword actions of creatures: "it explores", "it connives", "target creature you control explores, then it explores again"
//! (`Effect::Explore`, `Effect::Connive`). The writer's side is an arm of the renderer.

use super::effects::{is_on_the_battlefield, target_object};
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::Ref;

/// The creature that does the action: the card ("~", and "it" when the trigger is about the card itself or the sentence
/// before spoke of it), or one target creature, which "it" names after.
fn creature(r: &mut Reader<'_>) -> Option<Ref> {
    if r.source() {
        return Some(Ref::This);
    }
    if r.it == Some(Ref::This) || r.acts_itself {
        if r.word("it") {
            return Some(Ref::This);
        }
    } else if let Some(it) = r.it.clone()
        && r.word("it")
    {
        return Some(it);
    }
    let what = target_object(r)?;
    is_on_the_battlefield(r, &what).then_some(())?;
    r.it = Some(what.clone());
    Some(what)
}

/// "~ explores.", "it explores." and "target creature you control explores, then it explores again.": one creature
/// explores once or twice.
pub fn explore(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let what = creature(r)?;
    r.need("explores")?;
    let mut effects = vec![Effect::Explore { what: what.clone() }];
    if r.attempt(again).is_some() {
        effects.push(Effect::Explore { what });
    }
    Some(effects)
}

/// "~ connives.", "it connives." and "target creature you control connives.": one creature connives.
pub fn connive(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let what = creature(r)?;
    r.need("connives")?;
    Some(vec![Effect::Connive { what }])
}

/// ", then it explores again".
fn again(r: &mut Reader<'_>) -> Option<()> {
    r.punct(',').then_some(())?;
    r.need("then it explores again")
}
