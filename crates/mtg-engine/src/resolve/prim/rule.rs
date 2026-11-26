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
    } else if *duration == Duration::R113UntilTheirNextEndStep {
        // "Until that player's next end step, they may play that card": as long as theirs until their next end step.
        let who = mtg_cards::r5514_players::r113_permitted_player(rule).ok_or(
            ResolveError::Unsupported("Rule.Duration.R113UntilTheirNextEndStep"),
        )?;
        let Some(player) = cx.scope.players(who)?.first().copied() else {
            return Ok(Run::events(Vec::new()));
        };
        (&Duration::UntilYourNextEndStep, player)
    } else {
        (duration, cx.controller())
    };
    // An airbend ends with the rule that lets the owners cast what it exiled; the player airbent even when it exiled
    // nothing (Avatar Aang ruling).
    let airbent = matches!(rule, GameRule::MayCastRather { .. }).then(|| Event::Bent {
        player: cx.controller(),
        element: crate::events::Bending::Air,
    });
    let rule = lower_rule(&cx.scope, &fixed_choices(&cx.scope, rule))?;
    if names_nothing(&rule) {
        return Ok(Run::events(airbent.into_iter().collect()));
    }
    let controller = r5502_held_by(cx, &rule, duration, controller);
    // "During its controller's next turn" of a requirement to attack is the next turn of the players it waits for.
    let duration = match (&rule, duration) {
        (crate::rules::Rule::MustAttackAt { .. }, Duration::DuringItsControllersNextTurn) => {
            Duration::TheirNextTurn
        }
        (_, other) => other.clone(),
    };
    let duration = crate::effects::r109_mark_duration(state, &duration);
    let created = Event::RuleCreated {
        effect: Box::new(RuleEffect {
            id: EffectId(state.next_effect),
            rule,
            turn: crate::effects::began_turn(state, &duration, controller),
            duration,
            source: Some(cx.scope.source),
            controller,
        }),
    };
    Ok(Run::events(
        std::iter::once(created).chain(airbent).collect(),
    ))
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

/// The granted ability with each `SameCard` it names fixed to the object its name holds now (CR 611.2c): "that creature
/// gains 'Whenever this creature deals combat damage to a player, copy the exiled card'" keeps naming the card this
/// resolution exiled after the resolution is over.
pub(super) fn r6112_fixed_ability(
    scope: &Scope<'_>,
    granted: &std::sync::Arc<mtg_cards::ability::Ability>,
) -> std::sync::Arc<mtg_cards::ability::Ability> {
    fn walk(scope: &Scope<'_>, value: &mut Value) -> bool {
        match value {
            Value::Object(fields) => {
                if fields.len() == 1
                    && let Some(named) = fields.get("SameCard")
                    && let Ok(named) = serde_json::from_value::<mtg_cards::refs::Ref>(named.clone())
                    && !matches!(named, mtg_cards::refs::Ref::R111Object(_))
                    && let Ok([object]) = scope.objects(&named).as_deref()
                {
                    fields.insert(
                        "SameCard".to_owned(),
                        serde_json::json!({ "R111Object": object.id }),
                    );
                    return true;
                }
                let mut any = false;
                for inner in fields.values_mut() {
                    any |= walk(scope, inner);
                }
                any
            }
            Value::Array(items) => items
                .iter_mut()
                .fold(false, |any, item| walk(scope, item) | any),
            _ => false,
        }
    }
    let Ok(mut value) = serde_json::to_value(&**granted) else {
        return std::sync::Arc::clone(granted);
    };
    if !walk(scope, &mut value) {
        return std::sync::Arc::clone(granted);
    }
    serde_json::from_value(value)
        .map_or_else(|_| std::sync::Arc::clone(granted), std::sync::Arc::new)
}

/// Writes the chosen values into `value`; whether it wrote any.
pub(super) fn fix(scope: &Scope<'_>, value: &mut Value) -> bool {
    match value {
        Value::Object(fields) => {
            // "Protection from the chosen player": the player the source chose, fixed as the rule begins.
            if fields.len() == 1
                && fields.get("ControlledBy").and_then(Value::as_str) == Some("Chosen")
                && let Some(Chosen::Player(player)) = scope.entered_choice()
                && let Some(seat) = super::continuous::r110_seat_of(*player)
            {
                fields.insert(
                    "ControlledBy".to_owned(),
                    serde_json::json!({ "R110Seat": seat }),
                );
                return true;
            }
            if fields.len() == 1
                && let Some(name) = fields.get("ChosenAs").and_then(Value::as_str)
                && let Some(pred) = scope.chosen_value(name).and_then(as_pred)
                && let Ok(written) = serde_json::to_value(pred)
            {
                *value = written;
                return true;
            }
            // "That copy costs {2} less to cast": the object a name holds now, fixed as the rule begins (CR 611.2c).
            if fields.len() == 1
                && let Some(named) = fields.get("SameCard")
                && let Ok(named) = serde_json::from_value::<mtg_cards::refs::Ref>(named.clone())
                && !matches!(named, mtg_cards::refs::Ref::R111Object(_))
                && let Ok([object]) = scope.objects(&named).as_deref()
            {
                fields.insert(
                    "SameCard".to_owned(),
                    serde_json::json!({ "R111Object": object.id }),
                );
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
        // "The next spell you cast this turn of the chosen type": the card type the source chose as it entered.
        Value::String(word) if word == "ChosenSubtype" => {
            let Some(Chosen::CardType(kind)) = scope.entered_choice() else {
                return false;
            };
            match serde_json::to_value(Pred::Type(*kind)) {
                Ok(written) => {
                    *value = written;
                    true
                }
                Err(_) => false,
            }
        }
        _ => false,
    }
}

/// The predicate a chosen characteristic stands for.
fn as_pred(chosen: &Chosen) -> Option<Pred> {
    Some(match chosen {
        Chosen::Name(name) => Pred::Named(name.clone()),
        Chosen::Color(color) => Pred::Color(*color),
        Chosen::CreatureType(subtype)
        | Chosen::LandType(subtype)
        | Chosen::R6700PlaneswalkerType(subtype) => Pred::Subtype(subtype.clone()),
        Chosen::CardType(kind) => Pred::Type(*kind),
        _ => return None,
    })
}
