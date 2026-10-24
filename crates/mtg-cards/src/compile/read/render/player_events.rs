//! The event of a trigger that is one of several things a player does to a card: "Whenever you cycle or discard a card".

use super::Ref;
use crate::trigger::TriggerPattern;

/// The subject of the verbs and the ending a third person gives them.
fn subject(by: &Ref) -> Option<(&'static str, &'static str)> {
    match by {
        Ref::You => Some(("you", "")),
        Ref::Opponents => Some(("an opponent", "s")),
        Ref::EachPlayer => Some(("a player", "s")),
        _ => None,
    }
}

/// "Whenever you cycle or discard a card": two or more different events of one player, each of any card.
pub(super) fn text(pattern: &TriggerPattern) -> Option<String> {
    let TriggerPattern::AnyOf(events) = pattern else {
        return None;
    };
    let mut player = None;
    let mut verbs: Vec<&str> = Vec::with_capacity(events.len());
    for event in events {
        let (by, verb) = match event {
            TriggerPattern::Cycled { what, by } if what.is_empty() => (by, "cycle"),
            TriggerPattern::Discarded { what, by } if what.is_empty() => (by, "discard"),
            _ => return None,
        };
        if *player.get_or_insert(by) != by || verbs.contains(&verb) {
            return None;
        }
        verbs.push(verb);
    }
    (verbs.len() >= 2).then_some(())?;
    let (who, ending) = subject(player?)?;
    let said: Vec<String> = verbs.iter().map(|verb| format!("{verb}{ending}")).collect();
    Some(format!("Whenever {who} {} a card", said.join(" or ")))
}
