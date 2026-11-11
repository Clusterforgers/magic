//! Wordings of the rmt4 card pack: an enters trigger of a Bargain card under "if it was bargained", and a token for
//! each opponent that's tapped and attacking that player.

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::Condition;

/// "When ~ enters, if it was bargained, ...": the optional additional cost of the permanent's own spell
/// (`Condition::AdditionalCostPaid`), which the writer words as the gift.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    bargained(ability, texts);
    for text in each_opponent_attacked(ability) {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

fn bargained(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when:
            TriggerPattern::ZoneChange {
                what,
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
        intervening_if: Some(Condition::AdditionalCostPaid),
        ..
    } = ability
    else {
        return;
    };
    if what.0.as_slice() != [Pred::This] {
        return;
    }
    let found: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            text.strip_prefix("When ~ enters, if the gift was promised, ")
                .map(|rest| format!("When ~ enters, if it was bargained, {rest}"))
        })
        .collect();
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// "For each opponent, create a 1/1 black Ninja creature token that's tapped and attacking that player.": a triggered
/// ability whose one effect repeats over the opponents a token that enters tapped and attacking the opponent of the pass.
/// It is worded from the same ability with the token attacking no one named.
fn each_opponent_attacked(ability: &Ability) -> Vec<String> {
    let Ability::Triggered { effects, .. } = ability else {
        return Vec::new();
    };
    let [
        Effect::RepeatEach {
            over: Ref::Opponents,
            bind,
            effects: inner,
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    let [
        Effect::CreateToken {
            tapped: true,
            attacking: true,
            against: Some(Ref::Bound(against)),
            blocking: None,
            bind: None,
            ..
        },
    ] = inner.as_slice()
    else {
        return Vec::new();
    };
    if against != bind {
        return Vec::new();
    }
    let mut plain = ability.clone();
    if let Ability::Triggered { effects, .. } = &mut plain {
        let mut token = inner.clone();
        if let Some(Effect::CreateToken { against, .. }) = token.first_mut() {
            *against = None;
        }
        *effects = token;
    }
    let mut found = Vec::new();
    for text in super::render::written_variants(&plain) {
        let Some((head, rest)) = text.split_once(", create ") else {
            continue;
        };
        let Some(token) = rest.strip_suffix(" that's tapped and attacking.") else {
            continue;
        };
        for (subject, player) in [("", "player"), ("you ", "opponent")] {
            found.push(format!(
                "{head}, for each opponent, {subject}create {token} that's tapped and attacking that {player}."
            ));
        }
    }
    found
}
