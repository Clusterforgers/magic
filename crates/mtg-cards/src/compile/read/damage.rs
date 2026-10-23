//! Damage: "~ deals 3 damage to any target.", "~ deals 2 damage to each creature and each player." (two recipients that
//! are no targets, one `DealDamage` for each) and "~ deals 3 damage to any target and you gain 3 life." (the life gained
//! follows the damage). The source may be called "it" where the ability is about the card itself.

use super::amounts::points;
use super::defining;
use super::effects::life;
use super::selectors::{each, target};
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::Amount;

/// Who or what takes damage without being a target: "each opponent", "each player", "each creature", "that player", and
/// "you" where `you` allows it.
fn untargeted(r: &mut Reader<'_>, you: bool) -> Option<Ref> {
    if r.words("each opponent") {
        return Some(Ref::Opponents);
    }
    if r.words("each player") {
        return Some(Ref::EachPlayer);
    }
    if you && r.word("you") {
        return Some(Ref::You);
    }
    if r.that_player && r.words("that player") {
        return Some(Ref::Triggering);
    }
    each(r)
}

/// Who or what takes damage: a single target, or an untargeted recipient.
fn recipient(r: &mut Reader<'_>) -> Option<Ref> {
    if let Some(to) = untargeted(r, true) {
        return Some(to);
    }
    r.attempt(|r| {
        let to = target(r)?;
        let spec = r.targets.first()?;
        (spec.bounds.is_one() && spec.divide.is_none()).then_some(to)
    })
}

/// "and you gain 3 life", the life that follows the damage.
fn gain(r: &mut Reader<'_>) -> Option<Effect> {
    r.need("and you gain")?;
    Some(Effect::GainLife {
        who: Ref::You,
        amount: life(r)?,
    })
}

/// How much damage and who takes it: "3 damage to any target", "damage equal to the number of lands you control to any
/// target" and "damage to target creature equal to the number of lands you control".
fn amount_and_recipient(r: &mut Reader<'_>) -> Option<(Amount, Ref)> {
    let before = r.attempt(|r| {
        r.need("damage")?;
        let amount = defining::equal_to(r)?;
        r.need("to")?;
        Some((amount, recipient(r)?))
    });
    let after = || {
        r.attempt(|r| {
            r.need("damage to")?;
            let to = recipient(r)?;
            Some((defining::equal_to(r)?, to))
        })
    };
    if let Some(found) = before.or_else(after) {
        return Some(found);
    }
    let amount = points(r)?;
    r.need("damage to")?;
    Some((amount, recipient(r)?))
}

/// "~ deals 3 damage to any target.", "~ deals 2 damage to each creature and each player.", "It deals 1 damage to any
/// target." as the first thing an ability does, where "it" is the source, and "~ deals 3 damage to any target and you
/// gain 3 life.".
pub(super) fn damage(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    (r.source() || (r.targets.is_empty() && r.word("it"))).then_some(())?;
    r.need("deals")?;
    let (amount, first) = amount_and_recipient(r)?;
    let mut recipients = vec![first];
    // Objects and players are no overlap; two groups of objects or two groups of players may be.
    if r.targets.is_empty()
        && let Some(second) = r.attempt(|r| {
            r.word("and").then_some(())?;
            let second = untargeted(r, false)?;
            (matches!(second, Ref::Each(_)) != matches!(recipients.first()?, Ref::Each(_)))
                .then_some(second)
        })
    {
        recipients.push(second);
    }
    let mut effects: Vec<Effect> = recipients
        .into_iter()
        .map(|to| Effect::DealDamage {
            source: None,
            to,
            amount: amount.clone(),
        })
        .collect();
    effects.extend(r.attempt(gain));
    Some(effects)
}
