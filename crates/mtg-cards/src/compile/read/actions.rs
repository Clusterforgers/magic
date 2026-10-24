//! Keyword actions of creatures: "it explores", "it connives", "target creature you control explores, then it explores again",
//! "goad target creature", "enchanted creature fights target creature" (`Effect::Explore`, `Effect::Connive`,
//! `Effect::Goad`, `Effect::Fight`). The writer's side is an arm of the renderer.

use super::effects::{attached, is_on_the_battlefield, object, target_object, up_to_one_object};
use super::selectors::second_target;
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::{Bounds, Pred, Ref, Targetable};
use crate::value::{Amount, StatKind};

/// The creature that does the action: the card ("~", and "it" when the trigger is about the card itself or the sentence
/// before spoke of it), or one target creature, which "it" names after.
fn creature(r: &mut Reader<'_>) -> Option<Ref> {
    if r.source() {
        return Some(Ref::This);
    }
    if r.it == Some(Ref::This) || r.acts_itself {
        // A person on a card ("Whenever Kang attacks, he connives.") is "he" for the card.
        if r.word("it") || r.word("he") {
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

/// "Attach it to target creature you control." (also "attach ~ to ..."): an Equipment or an Aura that enters attaches
/// itself to a creature, and the sentence after may speak of "that creature".
pub fn attach(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("attach")?;
    r.acts_itself.then_some(())?;
    (r.source() || r.word("it")).then_some(())?;
    r.need("to")?;
    let to = target_object(r)?;
    is_on_the_battlefield(r, &to).then_some(())?;
    r.it = Some(to.clone());
    Some(vec![Effect::Attach {
        what: Ref::This,
        to,
    }])
}

/// ", then it explores again".
fn again(r: &mut Reader<'_>) -> Option<()> {
    r.punct(',').then_some(())?;
    r.need("then it explores again")
}

/// Who fights: "enchanted creature", the card, "it" and "that creature" (the creature the sentence before or the event
/// named), or the first target of the clause: "target creature you control", "up to one target creature you control".
fn fighter(r: &mut Reader<'_>) -> Option<Ref> {
    if let Some(host) = attached(r) {
        return Some(host);
    }
    if r.source() {
        return Some(Ref::This);
    }
    if let Some(it) = r.it.clone()
        && (r.word("it") || r.words("that creature"))
    {
        return Some(it);
    }
    if r.acts_itself && r.word("it") {
        return Some(Ref::This);
    }
    r.targets.is_empty().then_some(())?;
    up_to_one_object(r).or_else(|| target_object(r))
}

/// Who is fought: one target creature, or up to one, which is the first target of the clause or the second after a
/// fighter that is a target.
fn fought(r: &mut Reader<'_>) -> Option<Ref> {
    if r.targets.is_empty() {
        return up_to_one_object(r).or_else(|| target_object(r));
    }
    r.attempt(|r| {
        let what = second_target(r)?;
        let spec = r.targets.get(1)?;
        let optional = spec.bounds == Bounds::up_to(Some(Amount::Fixed(1)));
        (matches!(spec.what, Targetable::Object(_)) && (spec.bounds.is_one() || optional))
            .then_some(what)
    })
}

/// "Enchanted creature fights up to one target creature you don't control.", "~ fights target creature", "Target
/// creature you control fights another target creature.", "It fights up to one target creature you don't control." and
/// "Then that creature fights target creature you don't control." (after a sentence about the target): one creature
/// fights another (`Effect::Fight`).
pub fn fight(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.word("then");
    let what = fighter(r)?;
    is_on_the_battlefield(r, &what).then_some(())?;
    r.need("fights")?;
    let with = fought(r)?;
    is_on_the_battlefield(r, &with).then_some(vec![Effect::Fight { what, with }])
}

/// "Target creature you control deals damage equal to its power to target creature an opponent controls.": one creature
/// deals damage to another equal to its own power (a bite, `Effect::DealDamage` from the first target).
pub fn bite(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.targets.is_empty().then_some(())?;
    let dealer = target_object(r)?;
    is_on_the_battlefield(r, &dealer).then_some(())?;
    r.need("deals damage equal to its power to")?;
    let hit = fought(r)?;
    is_on_the_battlefield(r, &hit).then_some(())?;
    Some(vec![Effect::DealDamage {
        source: Some(dealer.clone()),
        to: hit,
        amount: Amount::Stat {
            of: dealer,
            stat: StatKind::Power,
        },
    }])
}

/// "Goad target creature.", "goad target creature an opponent controls", "Goad all creatures you don't control." and
/// "goad it" (the creature the sentence before named): the creatures are goaded (`Effect::Goad`).
pub fn goad(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("goad")?;
    let what = match r.it.clone() {
        Some(it) if r.word("it") => it,
        _ => object(r)?,
    };
    // Only permanents are goaded: not a spell on the stack.
    let spell = r.targets.first().is_some_and(|spec| {
        matches!(&spec.what, Targetable::Object(selector) if selector.0.contains(&Pred::Spell))
    });
    if spell {
        return None;
    }
    if matches!(what, Ref::Target(_)) {
        r.it = Some(what.clone());
    }
    is_on_the_battlefield(r, &what).then_some(vec![Effect::Goad { what }])
}
