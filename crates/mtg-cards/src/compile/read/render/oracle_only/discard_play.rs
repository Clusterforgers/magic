//! A discarded card exiled and played: "Whenever you discard a card, you may exile that card from your graveyard. If you
//! do, you may play that card this turn."

use super::super::pattern_text;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::{CastPermission, Duration, GameRule};
use crate::refs::{Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;

/// Adds the wording of a trigger on a discard whose card may be exiled and then played for the turn.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: when @ TriggerPattern::Discarded { .. },
        limit: None,
        targets,
        effects,
        ..
    } = ability
    else {
        return;
    };
    let [
        Effect::May {
            who: Ref::You,
            effects: inner,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let [
        Effect::Exile {
            what: Ref::Triggering,
            bind: Some(exiled),
            duration: None,
        },
        Effect::Rule {
            rule: GameRule::MayCast(permission),
            duration: Duration::UntilEndOfTurn,
        },
    ] = inner.as_slice()
    else {
        return;
    };
    let wanted = CastPermission {
        who: Ref::You,
        what: Selector::default(),
        from: Some(ZoneKind::Exile),
        free: false,
        flash: false,
        only: Some(Ref::Bound(exiled.clone())),
        play: true,
    };
    if !targets.is_empty() || *permission != wanted {
        return;
    }
    if let Some(event) = pattern_text(when) {
        texts.push(format!(
            "{event}, you may exile that card from your graveyard. If you do, you may play that card this turn."
        ));
    }
}
