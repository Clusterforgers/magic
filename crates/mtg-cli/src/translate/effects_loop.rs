//! Effects that repeat for each player (`RepeatEach`) and that ask a player for a value (`ChooseColor`, `ChooseType`).

use mtg_cards::effect::MOST_PASSES;
use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, only, unmapped};
use super::effects::chain_at;
use super::valid::{defined, selector_any, selector_in, subject};
use crate::forge::script::Params;

/// `ChooseCard`: the chooser picks cards that match, optionally or at random; the later lines read them as `ChosenCard`.
pub fn choose_card(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "ChooseCard",
        &[
            "Defined",
            "ValidTgts",
            "Choices",
            "ChoiceZone",
            "Amount",
            "MinAmount",
            "Mandatory",
            "AtRandom",
            "ControlledByPlayer",
            "Reveal",
            "RememberChosen",
        ],
    )?;
    // "Target opponent chooses a creature they control": the one player the line targets is the chooser, declared before the
    // pool that names them is read.
    let chooser = match p.get("ValidTgts") {
        None => None,
        Some("Player" | "Opponent" | "Player.Opponent")
            if !p.has("Defined")
                && !p.has("TargetMin")
                && !p.has("TargetMax")
                && p.get("AtRandom") != Some("True") =>
        {
            Some(subject(ctx, p, "You", None)?)
        }
        Some(_) => return unmapped("key:ChooseCard.ValidTgts"),
    };
    let mut filter = selector_in(ctx, p.get("Choices").unwrap_or("Card"), p.get("ChoiceZone"))?;
    match p.get("ControlledByPlayer") {
        Some("Remembered") => {
            let who = defined(ctx, "Remembered")?;
            filter.push(json!({"ControlledBy": who}));
        }
        Some(other) => return unmapped(format!("key:ChooseCard.ControlledByPlayer={other}")),
        None => {}
    }
    let from = json!({"Each": filter});
    let bind = ctx.fresh("card");
    let count = amount(ctx, p.get("Amount").unwrap_or("1"))?;
    let effect = if p.get("AtRandom") == Some("True") {
        if p.has("MinAmount") || p.get("Defined").is_some_and(|who| who != "You") {
            return unmapped("key:ChooseCard.AtRandom-with-chooser");
        }
        json!({"ChooseAtRandom": {"from": from, "count": count, "as": bind}})
    } else {
        let who = match chooser {
            Some(target) => target,
            None => defined(ctx, p.get("Defined").unwrap_or("You"))?,
        };
        let min = match (p.get("Mandatory"), p.get("MinAmount")) {
            (Some("True"), None) => count.clone(),
            (Some("True"), Some(min)) => amount(ctx, min)?,
            _ => json!(0),
        };
        json!({"Choose": {"who": who, "from": from, "count": {"min": min, "max": count}, "as": bind}})
    };
    ctx.chosen_card = Some(bind.clone());
    if p.get("RememberChosen") == Some("True") {
        // A pass of a loop binds its names anew, so what it remembers cannot outlive the pass or add up over passes.
        if ctx.remembered.is_some() {
            return unmapped("key:ChooseCard.RememberChosen-with-remembered");
        }
        ctx.remembered = Some(json!({ "Bound": bind }));
    }
    let mut effects = vec![effect];
    if p.get("Reveal") == Some("True") {
        effects.push(json!({"Reveal": {"what": {"Bound": bind}}}));
    }
    Ok(effects)
}

/// `RepeatEach` over players: the sub-ability once for each, with the player bound as `Remembered`.
pub fn repeat_each(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "RepeatEach",
        &[
            "RepeatPlayers",
            "RepeatCards",
            "Zone",
            "RepeatSubAbility",
            "DamageMap",
            "ChangeZoneTable",
            "UseImprinted",
        ],
    )?;
    let over = match (p.get("RepeatPlayers"), p.get("RepeatCards")) {
        (None, Some(valid)) => json!({"Each": selector_in(ctx, valid, p.get("Zone"))?}),
        (Some(_), Some(_)) => return unmapped("key:RepeatEach.RepeatPlayers+RepeatCards"),
        (players, None) => match players {
            Some("Player") => json!("EachPlayer"),
            Some("Player.Opponent" | "Opponent") => json!("Opponents"),
            Some("Targeted") => ctx.last_target()?,
            Some(other) => return unmapped(format!("key:RepeatEach.RepeatPlayers={other}")),
            None => return unmapped("key:RepeatEach.no-RepeatPlayers"),
        },
    };
    let Some(sub) = p.get("RepeatSubAbility") else {
        return unmapped("key:RepeatEach.no-RepeatSubAbility");
    };
    let bind = ctx.fresh("each");
    // `UseImprinted$` names the one the pass is for `Imprinted`, so `Remembered` keeps what it held before the loop.
    let held = json!({ "Bound": bind });
    let imprint = p.get("UseImprinted") == Some("True");
    let (outer, outer_imprinted) = (ctx.remembered.clone(), ctx.imprinted.clone());
    if imprint {
        ctx.imprinted = Some(held);
    } else {
        ctx.remembered = Some(held);
    }
    let part = ctx.sub(sub)?;
    let effects = chain_at(ctx, &part, false);
    (ctx.remembered, ctx.imprinted) = (outer, outer_imprinted);
    Ok(vec![
        json!({"RepeatEach": {"over": over, "as": bind, "effects": effects?}}),
    ])
}

/// `ImmediateTrigger`: a reflexive trigger that runs the named ability with targets of its own. `RememberObjects$` names
/// the objects the trigger is about, bound now (the trigger has targets of its own, so a `Target` would be another one)
/// and read by `DelayTriggerRemembered` in its steps.
pub fn immediate_trigger(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "ImmediateTrigger",
        &["Execute", "UnlessSwitched", "RememberObjects"],
    )?;
    let Some(name) = p.get("Execute") else {
        return unmapped("key:ImmediateTrigger.no-Execute");
    };
    let part = ctx.sub(name)?;
    let mut out = Vec::new();
    let mut bound = None;
    if let Some(word) = p.get("RememberObjects") {
        let what = match word {
            // A target an earlier step moved is another object now (CR 400.7), so `Targeted` stays a gap.
            "Remembered" | "RememberedCard" | "RememberedLKI" | "ChosenCard" | "ChosenPlayer" => {
                defined(ctx, word)?
            }
            w if super::triggered::is_word(w) => defined(ctx, w)?,
            other => return unmapped(format!("key:ImmediateTrigger.RememberObjects={other}")),
        };
        let bind = ctx.fresh("when");
        out.push(json!({"Bind": {"what": what, "as": bind}}));
        bound = Some(json!({ "Bound": bind }));
    }
    let outer_delayed = std::mem::replace(&mut ctx.delayed_remembered, bound);
    let outer = std::mem::take(&mut ctx.targets);
    let parent = ctx.parent_target.take();
    let effects = chain_at(ctx, &part, false);
    let targets = std::mem::replace(&mut ctx.targets, outer);
    ctx.parent_target = parent;
    ctx.delayed_remembered = outer_delayed;
    let mut reflexive = json!({"Reflexive": {"effects": effects?}});
    if !targets.is_empty() {
        reflexive["Reflexive"]["targets"] = Value::Array(targets);
    }
    out.push(reflexive);
    Ok(out)
}

/// `PeekAndReveal`: look at the top cards of a library and reveal all of them or the matching ones the player picks.
pub fn peek_and_reveal(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "PeekAndReveal",
        &[
            "PeekAmount",
            "RevealValid",
            "RevealOptional",
            "RememberRevealed",
            "RememberPeeked",
            "NoPeek",
            "NoReveal",
            "Defined",
        ],
    )?;
    if p.get("Defined").is_some_and(|who| who != "You") {
        return unmapped("key:PeekAndReveal.Defined");
    }
    let top = ctx.fresh("top");
    let from_top = json!({ "Bound": top });
    let count = amount(ctx, p.get("PeekAmount").unwrap_or("1"))?;
    let look = json!({"LookAtTop": {"count": count, "as": top}});
    let mut effects = vec![look];
    let revealed = match (p.get("NoReveal"), p.get("RevealValid")) {
        (Some("True"), None) => {
            if p.get("RememberPeeked") == Some("True") {
                ctx.remembered = Some(from_top);
            }
            return Ok(effects);
        }
        (Some("True"), Some(_)) => return unmapped("key:PeekAndReveal.NoReveal-with-RevealValid"),
        (_, None) => from_top,
        (_, Some(valid)) => {
            let mut filter = vec![json!({"In": "Library"}), json!({"Is": from_top})];
            filter.extend(selector_any(ctx, valid)?);
            let min = u8::from(p.get("RevealOptional") != Some("True"));
            let bind = ctx.fresh("revealed");
            effects.push(json!({"Choose": {"from": {"Each": filter}, "count": {"min": min, "max": 1}, "as": bind}}));
            json!({ "Bound": bind })
        }
    };
    effects.push(json!({"Reveal": {"what": revealed}}));
    if p.get("RememberRevealed") == Some("True") {
        ctx.remembered = Some(revealed);
    }
    Ok(effects)
}

/// `RollDice` of one die: the rows of `ResultSubAbilities` are the table of the roll.
pub fn roll_dice(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "RollDice",
        &[
            "Sides",
            "Amount",
            "ResultSubAbilities",
            "ResultSVar",
            "Defined",
        ],
    )?;
    if p.get("Amount").is_some_and(|dice| dice != "1") {
        return unmapped("key:RollDice.Amount");
    }
    let who = defined(ctx, p.get("Defined").unwrap_or("You"))?;
    // Forge rolls a six-sided die unless the line says otherwise.
    let Some(sides) = p.get("Sides").unwrap_or("6").parse::<u32>().ok() else {
        return unmapped("key:RollDice.Sides");
    };
    let mut rows = Vec::new();
    if let Some(name) = p.get("ResultSVar") {
        ctx.die_result = Some(name.to_owned());
    }
    // A roll whose result is only read as a number has no table: `chain_at` puts the lines after it into one row.
    let table = match (p.get("ResultSubAbilities"), p.has("ResultSVar")) {
        (Some(table), _) => table,
        (None, true) => "",
        (None, false) => return unmapped("effect:RollDice"),
    };
    for row in table.split(',').filter(|row| !row.is_empty()) {
        let Some((range, name)) = row.split_once(':') else {
            return unmapped("key:RollDice.ResultSubAbilities-row");
        };
        let (from, to) = match range.split_once('-') {
            Some((from, to)) => (from.parse::<u32>(), to.parse::<u32>()),
            None => (range.parse::<u32>(), range.parse::<u32>()),
        };
        let (Ok(from), Ok(to)) = (from, to) else {
            return unmapped("key:RollDice.ResultSubAbilities-range");
        };
        let part = ctx.sub(name)?;
        let effects = chain_at(ctx, &part, false)?;
        rows.push(json!({"from": from, "to": to, "effects": effects}));
    }
    Ok(vec![
        json!({"RollDice": {"who": who, "sides": sides, "results": rows}}),
    ])
}

/// `ChooseColor` and `ChooseType`: the chooser picks a value that the later lines of the ability read as the chosen one.
pub fn choose_value(ctx: &mut Ctx, p: &Params, api: &str) -> Tr<Vec<Value>> {
    only(p, api, &["Defined", "Type"])?;
    let kind = match (api, p.get("Type")) {
        ("ChooseColor", None) => "Color",
        ("ChooseType", Some("Creature")) => "CreatureType",
        ("ChooseType", Some("Card")) => "CardType",
        ("ChooseType", Some("Basic Land" | "Land")) => "LandType",
        (_, other) => return unmapped(format!("key:{api}.Type={}", other.unwrap_or(""))),
    };
    let who = defined(ctx, p.get("Defined").unwrap_or("You"))?;
    let bind = ctx.fresh("chosen");
    ctx.chosen = Some(bind.clone());
    ctx.chosen_kind = Some(kind);
    Ok(vec![
        json!({"ChooseValue": {"who": who, "kind": json!(kind), "as": bind}}),
    ])
}

/// `ChooseNumber`: one player picks a whole number from `Min$` (0) to `Max$`, read when the choice is made; the later lines
/// read it as `ChosenNumber`. Without a `Max$` the range is open and stays a gap.
pub fn choose_number(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "ChooseNumber", &["Defined", "Min", "Max"])?;
    let Some(max) = p.get("Max") else {
        return unmapped("key:ChooseNumber.no-Max");
    };
    let who = defined(ctx, p.get("Defined").unwrap_or("You"))?;
    if matches!(who.as_str(), Some("Opponents" | "EachPlayer")) {
        return unmapped("key:ChooseNumber.Defined=group");
    }
    let (min, max) = (amount(ctx, p.get("Min").unwrap_or("0"))?, amount(ctx, max)?);
    let bind = ctx.fresh("number");
    ctx.chosen_number = Some(bind.clone());
    Ok(vec![
        json!({"ChooseValue": {"who": who, "kind": {"Number": [min, max]}, "as": bind}}),
    ])
}

/// `AddPhase`: extra phases after the current one. An additional combat comes after the combat it is made in
/// (`AfterPhase$ EndCombat`) or before an additional main phase (`FollowedBy$ Main2`); anything else stays a gap.
pub fn add_phase(p: &Params) -> Tr<Value> {
    only(p, "AddPhase", &["ExtraPhase", "AfterPhase", "FollowedBy"])?;
    let phases = match (
        p.get("ExtraPhase"),
        p.get("AfterPhase"),
        p.get("FollowedBy"),
    ) {
        (Some("Combat"), Some("EndCombat"), None) => json!(["Combat"]),
        (Some("Combat"), None, Some("Main2")) => json!(["Combat", "Main"]),
        (Some("Beginning"), None, None) => json!(["Beginning"]),
        (Some(phase), after, followed) => {
            let tail = format!(
                "{}{}",
                after.unwrap_or_default(),
                followed.unwrap_or_default()
            );
            return unmapped(format!("key:AddPhase.{phase}{tail}"));
        }
        (None, ..) => return unmapped("key:AddPhase.no-ExtraPhase"),
    };
    Ok(json!({"AddPhase": {"phases": phases}}))
}

/// `TwoPiles`: one player separates the cards into two piles, another chooses one; the parts for the chosen and the other
/// pile read their pile as `Remembered`.
pub fn two_piles(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "TwoPiles",
        &[
            "Defined",
            "DefinedCards",
            "Separator",
            "Chooser",
            "ChosenPile",
            "UnchosenPile",
        ],
    )?;
    if !matches!(p.get("Defined"), None | Some("You")) {
        return unmapped("key:TwoPiles.Defined");
    }
    let (Some(cards), Some(separator), Some(chooser), Some(chosen)) = (
        p.get("DefinedCards"),
        p.get("Separator"),
        p.get("Chooser"),
        p.get("ChosenPile"),
    ) else {
        return unmapped("key:TwoPiles.shape");
    };
    let one_player = |word: &str, key: &str| -> Tr<Value> {
        let who = defined(ctx, word)?;
        // The engine takes the first opponent in turn order; everyone at once is no single player.
        if who.as_str() == Some("EachPlayer") {
            return unmapped(format!("key:TwoPiles.{key}={word}"));
        }
        Ok(who)
    };
    let who = one_player(separator, "Separator")?;
    let chooser = one_player(chooser, "Chooser")?;
    let what = defined(ctx, cards)?;
    let (chosen_as, rest_as) = (ctx.fresh("pile"), ctx.fresh("rest"));
    let mut effects = vec![json!({"SeparatePiles": {
        "who": who, "what": what, "chooser": chooser, "as": chosen_as, "rest_as": rest_as}})];
    let outer = ctx.remembered.clone();
    for (name, bound) in [
        (Some(chosen), &chosen_as),
        (p.get("UnchosenPile"), &rest_as),
    ] {
        if let Some(name) = name {
            ctx.remembered = Some(json!({ "Bound": bound }));
            let part = ctx.sub(name)?;
            effects.extend(chain_at(ctx, &part, false)?);
        }
    }
    ctx.remembered = outer;
    Ok(effects)
}

/// `Repeat` with a fixed number of passes (`MaxRepeat`): the body again and again, at most that many times; any other
/// end (a check on a remembered value, a player's choice) is a gap, so a loop always has a defined end.
pub fn repeat(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "Repeat", &["RepeatSubAbility", "MaxRepeat"])?;
    let Some(sub) = p.get("RepeatSubAbility") else {
        return unmapped("key:Repeat.no-RepeatSubAbility");
    };
    let Some(max) = p.get("MaxRepeat") else {
        return unmapped("key:Repeat.no-MaxRepeat");
    };
    let outer = ctx.remembered.clone();
    let part = ctx.sub(sub)?;
    let effects = chain_at(ctx, &part, false);
    ctx.remembered = outer;
    let effects = effects?;
    let mut repeat =
        json!({"condition": {"Compare": {"left": 0, "op": "Eq", "right": 0}}, "effects": effects});
    match max.parse::<u32>() {
        Ok(n) if (1..=MOST_PASSES).contains(&n) => repeat["most"] = json!(n),
        Ok(_) => return unmapped("key:Repeat.MaxRepeat=out-of-range"),
        Err(_) => {
            repeat["most"] = json!(MOST_PASSES);
            repeat["times"] = amount(ctx, max)?;
        }
    }
    Ok(vec![json!({"RepeatWhile": repeat})])
}
