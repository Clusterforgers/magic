//! r30-7c trigger heads no other writer gives: "Whenever ~ attacks a player and isn't blocked", "Whenever you cast a spell
//! from exile and whenever a permanent you control enters from exile", "Whenever one or more cards are put into exile from
//! graveyards and/or the battlefield during your turn".

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

/// Whether the pattern is a batch of cards put into exile from graveyards and/or the battlefield: any card from a
/// graveyard, a nontoken permanent from the battlefield.
fn r307c_exiled_from_graveyards_or_battlefield(pattern: &TriggerPattern) -> bool {
    let TriggerPattern::Batch(inner) = pattern else {
        return false;
    };
    let TriggerPattern::AnyOf(members) = &**inner else {
        return false;
    };
    let graveyard = TriggerPattern::ZoneChange {
        what: crate::refs::Selector::default(),
        from: Some(ZoneKind::Graveyard),
        to: Some(ZoneKind::Exile),
    };
    let battlefield = TriggerPattern::ZoneChange {
        what: crate::refs::Selector(vec![Pred::Not(Box::new(Pred::Token))]),
        from: Some(ZoneKind::Battlefield),
        to: Some(ZoneKind::Exile),
    };
    members.as_slice() == [graveyard, battlefield]
}

/// The whole head of a trigger with its condition, when this pack words them together: the exile of cards "during your
/// turn" is the event checked on your turn.
pub(super) fn r307c_head(
    when: &TriggerPattern,
    condition: Option<&crate::value::Condition>,
) -> Option<String> {
    let words =
        "Whenever one or more cards are put into exile from graveyards and/or the battlefield";
    match condition {
        Some(crate::value::Condition::YourTurn)
            if r307c_exiled_from_graveyards_or_battlefield(when) =>
        {
            Some(format!("{words} during your turn,"))
        }
        None if r307c_exiled_from_graveyards_or_battlefield(when) => Some(format!("{words},")),
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
