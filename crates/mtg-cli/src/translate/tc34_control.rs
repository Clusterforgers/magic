//! Control of a player and of a spell, and text changes: `ControlPlayer` (CR 722), `ControlSpell` with `Mode$ Gain`,
//! `ChangeText` (CR 612) and the `GenericChoice` between a color word and a basic land type change (Mind Bend).
//! `RestartGame` (Karn Liberated) stays a gap.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, only, unmapped};
use super::valid::subject;
use crate::forge::script::Params;

/// The effects of a verb this module reads; `None` for any other API.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Option<Tr<Vec<Value>>> {
    match api {
        "ControlPlayer" => Some(control_player(ctx, p).map(|node| vec![node])),
        "ControlSpell" => Some(control_spell(ctx, p).map(|node| vec![node])),
        "ChangeText" => Some(change_text(ctx, p, None)),
        "GenericChoice" if word_choice(ctx, p) => Some(either_word(ctx, p)),
        _ => None,
    }
}

/// `ControlPlayer`: the controller controls the target player during that player's next turn.
fn control_player(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(p, "ControlPlayer", &["ValidTgts", "Defined"])?;
    let who = subject(ctx, p, "Opponent", None)?;
    Ok(json!({"ControlPlayer": {"who": who}}))
}

/// A `ControlPlayer` line whose sub-ability is a delayed trigger that gives the controlled player one extra turn after
/// the controlled one (Emrakul, the Promised End): the line read without it, with `extra_turn`. Any other delayed
/// trigger after a `ControlPlayer` is a gap; `None` for every other line.
pub fn control_with_extra_turn(ctx: &mut Ctx, p: &Params, top: bool) -> Option<Tr<Vec<Value>>> {
    if p.get("DB").or_else(|| p.get("SP")).or_else(|| p.get("AB")) != Some("ControlPlayer") {
        return None;
    }
    let next = Params::parse(ctx.svar(p.get("SubAbility")?)?);
    if next.get("DB") != Some("DelayedTrigger") {
        return None;
    }
    if !extra_turn_after(ctx, &next) {
        return Some(unmapped("key:ControlPlayer.extra-turn"));
    }
    Some(
        super::effects::chain_at(ctx, &p.without(&["SubAbility"]), top).map(|mut effects| {
            for effect in &mut effects {
                if let Some(control) = effect.get_mut("ControlPlayer") {
                    control["extra_turn"] = json!(true);
                }
            }
            effects
        }),
    )
}

/// Whether a delayed trigger gives the controlled player one extra turn after the controlled one: at that turn's
/// cleanup step, `AddTurn | NumTurns$ 1` for the player it remembers.
fn extra_turn_after(ctx: &Ctx, delayed: &Params) -> bool {
    let Some(add) = delayed.get("Execute").and_then(|name| ctx.svar(name)) else {
        return false;
    };
    let add = Params::parse(add);
    delayed.get("Mode") == Some("Phase")
        && delayed.get("Phase") == Some("Cleanup")
        && delayed.get("DelayedTriggerDefinedPlayer") == Some("Targeted")
        && delayed.get("SubAbility").is_none()
        && add.get("DB") == Some("AddTurn")
        && add.get("NumTurns") == Some("1")
        && add.get("Defined") == Some("DelayTriggerRemembered")
        && add.get("SubAbility").is_none()
}

/// `ControlSpell | Mode$ Gain`: the controller gains control of the target spell for good.
fn control_spell(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(p, "ControlSpell", &["ValidTgts", "TargetType", "Mode"])?;
    if p.get("Mode") != Some("Gain") {
        return unmapped("key:ControlSpell.Mode");
    }
    if p.get("TargetType") != Some("Spell") {
        return unmapped("key:ControlSpell.TargetType");
    }
    // The target is a card on the stack: each alternative of the valid-string gains the zone.
    let Some(valid) = p.get("ValidTgts") else {
        return unmapped("key:ControlSpell.no-target");
    };
    let on_stack: Vec<String> = valid
        .split(',')
        .map(|alternative| match alternative.split_once('.') {
            Some((base, props)) => format!("{base}.{props}+inZoneStack"),
            None => format!("{alternative}.inZoneStack"),
        })
        .collect();
    let target = p
        .without(&["ValidTgts"])
        .with("ValidTgts", &on_stack.join(","));
    let what = subject(ctx, &target, "Self", None)?;
    Ok(json!({"Continuous": {
        "affected": what,
        "mods": [{"SetController": "You"}],
        "duration": "Forever",
    }}))
}

/// The kind of word a `ChangeText` line replaces and the kind it writes, as `ChooseValue` kinds.
fn kinds(p: &Params) -> Tr<(Value, Value)> {
    let like = json!({"LikeChosen": "from"});
    match (p.get("ChangeColorWord"), p.get("ChangeTypeWord")) {
        (Some("Choose Choose"), None) => Ok((json!("Color"), like)),
        (None, Some("ChooseBasicLandType ChooseBasicLandType")) => {
            Ok((json!("BasicLandType"), like))
        }
        (None, Some("ChooseCreatureType ChooseCreatureType")) => match p.get("ForbiddenNewTypes") {
            None => Ok((json!("CreatureType"), like)),
            Some(left_out) if !left_out.contains(',') => Ok((
                json!("CreatureType"),
                json!({"CreatureTypeExcept": [left_out]}),
            )),
            Some(_) => unmapped("key:ChangeText.ForbiddenNewTypes"),
        },
        _ => unmapped("key:ChangeText.words"),
    }
}

/// The target of a text change (`TgtZone$ Stack,Battlefield` folds into "spell or permanent"), or the parent target.
fn changed(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    subject(ctx, p, "Self", None)
}

/// The steps of a text change: the word replaced and the word written are chosen, then the target's text changes for
/// good (`Duration$ Permanent`) or until end of turn. `kind` overrides the kind the line names (a choice between two).
fn change_text(ctx: &mut Ctx, p: &Params, kind: Option<Value>) -> Tr<Vec<Value>> {
    only(
        p,
        "ChangeText",
        &[
            "ValidTgts",
            "Defined",
            "ChangeColorWord",
            "ChangeTypeWord",
            "ForbiddenNewTypes",
            "Duration",
        ],
    )?;
    let (from, to) = kinds(p)?;
    let from = kind.unwrap_or(from);
    let duration = match p.get("Duration") {
        Some("Permanent") => "Forever",
        None => "UntilEndOfTurn",
        Some(_) => return unmapped("key:ChangeText.Duration"),
    };
    let what = changed(ctx, p)?;
    Ok(vec![
        json!({"ChooseValue": {"kind": from, "as": "from"}}),
        json!({"ChooseValue": {"kind": to, "as": "to"}}),
        json!({"Continuous": {
            "affected": what,
            "mods": [{"ChangeTextChosen": {"from": "from", "to": "to"}}],
            "duration": duration,
        }}),
    ])
}

/// The two parts of a `GenericChoice` that are text changes of the parent target: a color word and a basic land type.
fn word_parts(ctx: &Ctx, p: &Params) -> Option<(Params, Params)> {
    let names: Vec<&str> = p.get("Choices")?.split(',').collect();
    let [color, land] = names.as_slice() else {
        return None;
    };
    let color = Params::parse(ctx.svar(color)?);
    let land = Params::parse(ctx.svar(land)?);
    let text = |part: &Params| {
        part.get("DB") == Some("ChangeText") && part.get("Defined") == Some("ParentTarget")
    };
    (text(&color)
        && text(&land)
        && color.get("ChangeColorWord") == Some("Choose Choose")
        && land.get("ChangeTypeWord") == Some("ChooseBasicLandType ChooseBasicLandType")
        && color.get("Duration") == land.get("Duration")
        && color.get("SubAbility") == land.get("SubAbility"))
    .then_some((color, land))
}

/// Whether a `GenericChoice` is the choice between a color word and a basic land type change.
fn word_choice(ctx: &Ctx, p: &Params) -> bool {
    word_parts(ctx, p).is_some()
}

/// "replacing all instances of one color word with another or one basic land type with another": one text change whose
/// word is either kind, then what both parts do after it.
fn either_word(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "GenericChoice", &["Choices", "Defined", "ValidTgts"])?;
    if p.get("Defined").is_some_and(|who| who != "You") {
        return unmapped("key:GenericChoice.Defined");
    }
    let Some((color, _)) = word_parts(ctx, p) else {
        return unmapped("key:GenericChoice.Choices");
    };
    // The parent target is declared by the choice itself.
    let mut part = p.without(&["Choices", "Defined"]);
    for key in ["ChangeColorWord", "Duration"] {
        if let Some(value) = color.get(key) {
            part = part.with(key, value);
        }
    }
    let mut effects = change_text(ctx, &part, Some(json!("ColorOrBasicLandType")))?;
    if let Some(name) = color.get("SubAbility") {
        let next = ctx.sub(name)?;
        effects.extend(super::effects::chain_at(ctx, &next, false)?);
    }
    Ok(effects)
}
