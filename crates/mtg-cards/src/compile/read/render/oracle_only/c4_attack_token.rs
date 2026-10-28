//! A token that enters tapped and attacking and is exiled at end of combat: "Whenever ~ attacks, create a 4/4 white
//! Angel creature token with flying that's tapped and attacking. Exile that token at end of combat." The renderers word
//! the exile of a bound token only for a copy.

use super::super::variants;
use super::twin_json::twin;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::trigger::{Step, TriggerPattern};
use crate::value::Amount;

/// Adds the wording of a trigger that creates one token tapped and attacking and exiles it at end of combat.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { effects, .. } = ability else {
        return;
    };
    let [
        Effect::CreateToken {
            count,
            tapped: true,
            attacking: true,
            bind: Some(name),
            ..
        },
        Effect::DelayedTrigger {
            when:
                TriggerPattern::StepBegins {
                    step: Step::EndCombat,
                    whose: Ref::EachPlayer,
                },
            once: true,
            this_turn: false,
            effects: later,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let exiles_it = matches!(
        later.as_slice(),
        [Effect::Exile { what: Ref::Bound(bound), bind: None, duration: None }] if bound == name
    );
    if !exiles_it || *count != Amount::one() {
        return;
    }
    let plain = twin(ability, |value| {
        let effects = value
            .get_mut("Triggered")?
            .get_mut("effects")?
            .as_array_mut()?;
        effects.truncate(1);
        effects
            .first_mut()?
            .get_mut("CreateToken")?
            .as_object_mut()?
            .remove("as")?;
        Some(())
    });
    let Some(plain) = plain else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter(|text| text.ends_with(" that's tapped and attacking."))
        .map(|text| format!("{text} Exile that token at end of combat."))
        .collect();
    texts.extend(said);
}
