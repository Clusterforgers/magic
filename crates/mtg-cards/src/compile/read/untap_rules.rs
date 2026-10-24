//! A permanent that stays tapped: "This artifact doesn't untap during your untap step.", "Enchanted creature doesn't
//! untap during its controller's untap step [unless that player is the monarch]." Read into a static `DoesntUntap`
//! (under the monarch condition when there is one) and written back.

use super::statics::statics;
use super::tokens::Reader;
use crate::ability::Ability;
use crate::modify::Modification;
use crate::refs::Ref;
use crate::value::Condition;

/// The nouns an Aura or Equipment calls the permanent it is on.
const HOSTS: [&str; 4] = ["creature", "permanent", "land", "artifact"];

/// "~ doesn't untap during your untap step", "enchanted creature doesn't untap during its controller's untap step".
pub(super) fn doesnt_untap(r: &mut Reader<'_>) -> Option<Ability> {
    let (affected, own) = subject(r)?;
    r.need(if own {
        "doesn't untap during your untap step"
    } else {
        "doesn't untap during its controller's untap step"
    })?;
    let condition = r.attempt(|r| {
        r.need("unless")?;
        monarch(r, own)
    });
    Some(statics(
        affected,
        vec![Modification::DoesntUntap],
        condition,
    ))
}

/// The permanent and whether it is the card itself (whose controller is "you").
fn subject(r: &mut Reader<'_>) -> Option<(Ref, bool)> {
    if r.source() {
        return Some((Ref::This, true));
    }
    r.attempt(|r| {
        (r.word("enchanted") || r.word("equipped")).then_some(())?;
        HOSTS.iter().any(|host| r.word(host)).then_some(())?;
        Some((Ref::Attached, false))
    })
}

/// "that player is the monarch" (the controller of the permanent it is on) or "you're the monarch".
fn monarch(r: &mut Reader<'_>, own: bool) -> Option<Condition> {
    let who = if own {
        r.need("you're the monarch")?;
        Ref::You
    } else {
        r.need("that player is the monarch")?;
        Ref::ControllerOf(Box::new(Ref::Attached))
    };
    Some(Condition::Not(Box::new(Condition::Monarch(who))))
}

/// Every wording of the ability the reader makes, without the period.
pub(super) fn texts(
    affected: &Ref,
    mods: &[Modification],
    condition: Option<&Condition>,
) -> Vec<String> {
    if mods != [Modification::DoesntUntap] {
        return Vec::new();
    }
    let tail = match condition {
        None => "",
        Some(Condition::Not(held)) => match &**held {
            Condition::Monarch(Ref::You) if *affected == Ref::This => " unless you're the monarch",
            Condition::Monarch(Ref::ControllerOf(host))
                if **host == Ref::Attached && *affected == Ref::Attached =>
            {
                " unless that player is the monarch"
            }
            _ => return Vec::new(),
        },
        Some(_) => return Vec::new(),
    };
    match affected {
        Ref::This => vec![format!("~ doesn't untap during your untap step{tail}")],
        Ref::Attached => ["enchanted", "equipped"]
            .iter()
            .flat_map(|side| HOSTS.iter().map(move |host| format!("{side} {host}")))
            .map(|host| format!("{host} doesn't untap during its controller's untap step{tail}"))
            .collect(),
        _ => Vec::new(),
    }
}
