//! `Effect::Rule`: a rule of the game changes for a while (CR 611).

use mtg_cards::effect::Effect;
use mtg_cards::modify::{Duration, GameRule};
use mtg_cards::refs::Pred;
use mtg_core::ids::EffectId;
use mtg_core::ids::PlayerId;
use serde_json::Value;

use super::wrong_effect;
use crate::events::Event;
use crate::object::Chosen;
use crate::resolve::{Cx, ResolveError, Run, Scope};
use crate::rules::{RuleEffect, lower_rule, names_nothing};

/// The rule begins to apply to the players and objects its references name now (CR 611.2c) and lasts for its duration.
/// Naming no player and no object begins nothing. A rule the engine does not play refuses by name.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Rule { rule, duration } = effect else {
        return Err(wrong_effect("Rule"));
    };
    let state = cx.state();
    // "Until that player's next turn": the rule lasts as one until that player's own next turn would.
    let (duration, controller) = if *duration == Duration::UntilTheirNextTurn {
        let who = mtg_cards::r5514_players::r5514_rule_player(rule).ok_or(
            ResolveError::Unsupported("Rule.Duration.UntilTheirNextTurn"),
        )?;
        let Some(player) = cx.scope.players(who)?.first().copied() else {
            return Ok(Run::events(Vec::new()));
        };
        (&Duration::UntilYourNextTurn, player)
    } else {
        (duration, cx.controller())
    };
    let rule = lower_rule(&cx.scope, &fixed_choices(&cx.scope, rule))?;
    if names_nothing(&rule) {
        return Ok(Run::events(Vec::new()));
    }
    let controller = r5502_held_by(cx, &rule, duration, controller);
    // "During its controller's next turn" of a requirement to attack is the next turn of the players it waits for.
    let duration = match (&rule, duration) {
        (crate::rules::Rule::MustAttackAt { .. }, Duration::DuringItsControllersNextTurn) => {
            Duration::TheirNextTurn
        }
        (_, other) => other.clone(),
    };
    Ok(Run::events(vec![Event::RuleCreated {
        effect: Box::new(RuleEffect {
            id: EffectId(state.next_effect),
            rule,
            turn: crate::effects::began_turn(state, &duration, controller),
            duration,
            source: Some(cx.scope.source),
            controller,
        }),
    }]))
}

/// The player the rule is begun for: `begun_for` (the resolving controller, or the player of `UntilTheirNextTurn`), or for
/// `UntilItsControllersNextCombatEnds` the controller of the creature that must attack, whose combat it waits for.
fn r5502_held_by(
    cx: &Cx<'_>,
    rule: &crate::rules::Rule,
    duration: &Duration,
    begun_for: PlayerId,
) -> PlayerId {
    match (rule, duration) {
        (crate::rules::Rule::MustAttack(named), Duration::UntilItsControllersNextCombatEnds) => {
            named
                .first()
                .and_then(|first| crate::characteristics::controller_of(cx.state(), first.id))
                .unwrap_or(begun_for)
        }
        _ => begun_for,
    }
}

/// The rule with each value the resolution chose (`Pred::ChosenAs`) written in as what was chosen: "spells with the chosen
/// name can't be cast until your next turn" keeps the name after the resolution that chose it is over. A value that is no
/// characteristic, or one not chosen, is left as it is and matches nothing, as before.
fn fixed_choices(scope: &Scope<'_>, rule: &GameRule) -> GameRule {
    let Ok(mut value) = serde_json::to_value(rule) else {
        return rule.clone();
    };
    if !fix(scope, &mut value) {
        return rule.clone();
    }
    serde_json::from_value(value).unwrap_or_else(|_| rule.clone())
}

/// Writes the chosen values into `value`; whether it wrote any.
pub(super) fn fix(scope: &Scope<'_>, value: &mut Value) -> bool {
    match value {
        Value::Object(fields) => {
            if fields.len() == 1
                && let Some(name) = fields.get("ChosenAs").and_then(Value::as_str)
                && let Some(pred) = scope.chosen_value(name).and_then(as_pred)
                && let Ok(written) = serde_json::to_value(pred)
            {
                *value = written;
                return true;
            }
            let mut any = false;
            for inner in fields.values_mut() {
                any |= fix(scope, inner);
            }
            any
        }
        Value::Array(items) => {
            let mut any = false;
            for item in items {
                any |= fix(scope, item);
            }
            any
        }
        _ => false,
    }
}

/// The predicate a chosen characteristic stands for.
fn as_pred(chosen: &Chosen) -> Option<Pred> {
    Some(match chosen {
        Chosen::Name(name) => Pred::Named(name.clone()),
        Chosen::Color(color) => Pred::Color(*color),
        Chosen::CreatureType(subtype) | Chosen::LandType(subtype) => Pred::Subtype(subtype.clone()),
        Chosen::CardType(kind) => Pred::Type(*kind),
        _ => return None,
    })
}
