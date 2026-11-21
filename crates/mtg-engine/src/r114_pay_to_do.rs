//! Paying to do what a rule says: "Until end of turn, you may pay {1} any time you could cast an instant. If you do,
//! prevent the next 1 damage that would be dealt to that permanent or player this turn." (`GameRule::R114PayToDo`).
//! Paying is a special action of the rule's controller any time they have priority (CR 116.2): it does not use the stack,
//! it may be taken again while the rule lasts, and each payment does the rule's effects at once, `Target(0)` in them the
//! object or player the spell that began the rule targeted, as it was then (CR 611.2c).

use mtg_cards::effect::Effect;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use crate::choose::Target;
use crate::decision::R114PaidDo;
use crate::events::Event;
use crate::mana::{ManaOption, ManaPool, can_afford};
use crate::mana_takes::for_card;
use crate::rules::{Rule, RuleEffect};
use crate::spend::Spend;
use crate::state::GameState;

/// The name `Target(0)` of the effects is read as, bound to what the rule fixed.
const FIXED: &str = "~r114 fixed";

/// The rule effect `id`, while it is a rule to pay to do something that `player` holds.
fn held(state: &GameState, player: PlayerId, id: EffectId) -> Option<&RuleEffect> {
    state.rule_effects.iter().find(|effect| {
        effect.id == id
            && effect.controller == player
            && matches!(effect.rule, Rule::R114PayToDo { .. })
    })
}

/// The card whose spell began the rule and what each payment costs.
pub(crate) fn offer(state: &GameState, id: EffectId) -> Option<(ObjectId, ManaCost)> {
    let effect = state.rule_effects.iter().find(|effect| effect.id == id)?;
    let Rule::R114PayToDo { cost, .. } = &effect.rule else {
        return None;
    };
    Some((effect.source?.id, cost.clone()))
}

/// The rules `player` may pay to do now, when the mana they could make pays the cost.
pub(crate) fn payable(
    state: &GameState,
    player: PlayerId,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
) -> Vec<R114PaidDo> {
    state
        .rule_effects
        .iter()
        .filter(|effect| effect.controller == player)
        .filter_map(|effect| {
            let Rule::R114PayToDo { cost, .. } = &effect.rule else {
                return None;
            };
            let object = effect.source?.id;
            can_afford(
                cost,
                pool,
                &for_card(state, sources, object),
                life,
                &Spend::other().r111_by(state, player),
            )
            .then(|| R114PaidDo {
                effect: effect.id,
                object,
                cost: cost.clone(),
            })
        })
        .collect()
}

/// The events of what one payment does, run at once without the stack; none when `player` may not pay for `id` now or
/// the effects cannot run.
pub(crate) fn events(state: &GameState, player: PlayerId, id: EffectId) -> Option<Vec<Event>> {
    let effect = held(state, player, id)?;
    let Rule::R114PayToDo { effects, fixed, .. } = &effect.rule else {
        return None;
    };
    let source = effect.source?;
    let effects: Vec<Effect> = effects.iter().map(read_fixed).collect::<Option<_>>()?;
    let bound: Vec<(String, Vec<Target>)> = vec![(FIXED.to_owned(), fixed.clone())];
    crate::resolve::r114_run_bound(state, player, source, &effects, &bound).ok()
}

/// The effect with each `Target(0)` read as the name the fixed object or player is bound to.
fn read_fixed(effect: &Effect) -> Option<Effect> {
    let mut value = serde_json::to_value(effect).ok()?;
    rename(&mut value);
    serde_json::from_value(value).ok()
}

/// Writes `{"Bound": FIXED}` in place of every `{"Target": 0}`.
fn rename(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(fields) => {
            if fields.len() == 1 && fields.get("Target") == Some(&serde_json::json!(0)) {
                *value = serde_json::json!({ "Bound": FIXED });
                return;
            }
            fields.values_mut().for_each(rename);
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(rename),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    //! Goal: `Target(0)` in the effects of a rule to pay to do something is read as the fixed name wherever it stands, and
    //! nothing else is renamed.
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn target_zero_is_read_as_the_fixed_name_and_nothing_else() {
        let effect: Effect = serde_json::from_str(
            r#"{"Shield":{"when":{"Damage":{"to":{"Target":0}}},"by":{"PreventUpTo":1},"duration":"UntilEndOfTurn"}}"#,
        )
        .unwrap();
        let read = serde_json::to_string(&read_fixed(&effect).unwrap()).unwrap();
        assert!(read.contains(r#"{"Bound":"~r114 fixed"}"#), "{read}");
        let other: Effect =
            serde_json::from_str(r#"{"GainLife":{"who":{"Target":1},"amount":1}}"#).unwrap();
        assert_eq!(read_fixed(&other), Some(other));
    }
}
