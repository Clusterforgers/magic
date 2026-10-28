//! A step at end of combat on what the source dealt combat damage to: "Whenever this creature deals combat damage to a
//! creature, destroy that creature at end of combat." The head is the one `w5a_heads` derives; the step is the verb of
//! the one effect of a delayed trigger at the end of combat step, on the creature the trigger names.

use serde_json::{Value, json};

use super::super::super::selectors::type_name;
use super::w4b_stand::json as to_json;
use super::w5a_heads::event;
use crate::ability::Ability;
use crate::refs::{Pred, Ref};
use crate::trigger::TriggerPattern;

/// The verbs of the steps on the damaged object, by the effect's data.
const VERBS: [(&str, &str); 2] = [("Destroy", "destroy"), ("Exile", "exile")];

/// Adds the wording of a damage trigger whose one effect acts on the damaged object at end of combat.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when:
            when @ TriggerPattern::DealsDamage {
                to: Some(Ref::Each(hit)),
                combat: Some(true),
                ..
            },
        intervening_if: None,
        limit: None,
        targets,
        effects,
        ..
    } = ability
    else {
        return;
    };
    let [Pred::Type(kind)] = hit.0.as_slice() else {
        return;
    };
    let [effect] = effects.as_slice() else {
        return;
    };
    if !targets.is_empty() {
        return;
    }
    let Some((_, said)) = event(when) else {
        return;
    };
    let Some(verb) = later_verb(&to_json(effect)) else {
        return;
    };
    texts.push(format!(
        "{said}, {verb} that {} at end of combat.",
        type_name(*kind)
    ));
}

/// The verb of a delayed trigger at the end of combat whose one step acts on the triggering object.
fn later_verb(effect: &Value) -> Option<&'static str> {
    let delayed = effect.get("DelayedTrigger")?.as_object()?;
    let step = json!({"StepBegins": {"step": "EndCombat"}});
    let keys_known = delayed.keys().all(|key| key == "when" || key == "effects");
    (keys_known && delayed.get("when") == Some(&step)).then_some(())?;
    let [inner] = delayed.get("effects")?.as_array()?.as_slice() else {
        return None;
    };
    VERBS.iter().find_map(|(name, verb)| {
        (*inner == json!({ *name: {"what": "Triggering"} })).then_some(*verb)
    })
}
