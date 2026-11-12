//! r30-7c trigger heads no other writer gives: "Whenever ~ attacks a player and isn't blocked", "Whenever you cast a spell
//! from exile and whenever a permanent you control enters from exile".

use crate::refs::{Pred, Ref, ZoneKind};
use crate::trigger::TriggerPattern;

/// The words of one event from exile, after "whenever": a spell you cast from exile or a permanent you control entering
/// from exile.
fn r307c_from_exile(pattern: &TriggerPattern) -> Option<&'static str> {
    match pattern {
        TriggerPattern::SpellCast { what, by: Ref::You }
            if what.0 == [Pred::CastFrom(ZoneKind::Exile)] =>
        {
            Some("you cast a spell from exile")
        }
        TriggerPattern::ZoneChange {
            what,
            from: Some(ZoneKind::Exile),
            to: Some(ZoneKind::Battlefield),
        } if what.0 == [Pred::ControlledBy(Ref::You)] => {
            Some("a permanent you control enters from exile")
        }
        _ => None,
    }
}

/// The head of a trigger this pack writes, if it is one.
pub(super) fn r307c_pattern_text(pattern: &TriggerPattern) -> Option<String> {
    match pattern {
        TriggerPattern::Unblocked { what }
            if what.0 == [Pred::This, Pred::AttackingPlayer(Ref::EachPlayer)] =>
        {
            Some("Whenever ~ attacks a player and isn't blocked".to_owned())
        }
        TriggerPattern::AnyOf(patterns) => {
            let heads = patterns
                .iter()
                .map(r307c_from_exile)
                .collect::<Option<Vec<_>>>()?;
            (heads.len() > 1).then(|| format!("Whenever {}", heads.join(" and whenever ")))
        }
        _ => r307c_from_exile(pattern).map(|head| format!("Whenever {head}")),
    }
}

/// Whether the ability triggers on a batch of damage to players and acts on its dealers (`Ref::Dealer`), which a batch
/// names together: "those creatures".
fn r307c_on_dealers(ability: &crate::ability::Ability) -> bool {
    let crate::ability::Ability::Triggered { when, effects, .. } = ability else {
        return false;
    };
    let TriggerPattern::Batch(inner) = when else {
        return false;
    };
    let TriggerPattern::DealsDamage { to: Some(to), .. } = &**inner else {
        return false;
    };
    let to_players = matches!(to, Ref::EachPlayer | Ref::Opponents | Ref::You);
    let named = serde_json::to_string(effects).is_ok_and(|json| json.contains("\"Dealer\""));
    to_players && named
}

/// A batch of damage names its dealers together: "put a +1/+1 counter on each of those creatures and draw a card" for
/// what a single event words "on that creature, then draw a card"; the singular wording is dropped.
pub(super) fn r307c_wordings(ability: &crate::ability::Ability, texts: &mut Vec<String>) {
    if !r307c_on_dealers(ability) {
        return;
    }
    let mut plural = Vec::new();
    for text in texts.iter() {
        if !text.contains("on that creature") {
            continue;
        }
        let each = text.replace("on that creature", "on each of those creatures");
        let joined = each.replacen(", then draw ", " and draw ", 1);
        for wording in [each, joined] {
            if !plural.contains(&wording) {
                plural.push(wording);
            }
        }
    }
    texts.retain(|text| !text.contains("that creature"));
    texts.extend(plural);
}
