//! Effects that repeat for each player (`RepeatEach`) and that ask a player for a value (`ChooseColor`, `ChooseType`).

use mtg_cards::effect::MOST_PASSES;
use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, only, unmapped};
use super::effects::chain_at;
use super::valid::{defined, selector_any, selector_in, subject};
use crate::forge::script::Params;

/// What `Remembered` holds once a `ChooseCard` adds its pick to what earlier picks of the chain remembered ("choose a
/// creature card with mana value 1, then do the same for 2 and 3"): every pick, wherever it is now. Only picks of
/// `ChooseCard` lines that follow each other add up; anything else held (a loop's object or player, a pass of a loop that
/// would add up over passes) keeps the line a gap.
fn add_chosen(ctx: &Ctx, held: &Value, before: Option<&str>, bind: &str) -> Tr<Value> {
    let gap = || unmapped("key:ChooseCard.RememberChosen-with-remembered");
    let pick = json!({"Is": {"Bound": bind}});
    let picks = match (held, before) {
        (Value::Object(map), Some(name))
            if map.get("Bound").and_then(Value::as_str) == Some(name) =>
        {
            vec![json!({ "Is": held }), pick]
        }
        _ => match held.pointer("/Each/0/AnyOf").and_then(Value::as_array) {
            Some(earlier) if held.pointer("/Each/1") == Some(&chosen_zones()) => {
                let mut all = earlier.clone();
                all.push(pick);
                all
            }
            _ => return gap(),
        },
    };
    if ctx.player_loop.is_some() || super::memory::is_scoped(ctx, held) {
        return gap();
    }
    Ok(json!({"Each": [{"AnyOf": picks}, chosen_zones()]}))
}

/// The name the picks of a `ChooseCard` gather into when it ends the pass of a repeat over players and remembers its pick
/// beside the player of the pass: the repeat names every pick after it. A pick some later step of the pass reads stays a gap.
fn gathers(ctx: &mut Ctx, p: &Params) -> Option<String> {
    let player = ctx.player_loop.as_ref()?;
    if ctx.remembered != Some(json!({ "Bound": player })) || p.has("SubAbility") {
        return None;
    }
    if ctx.gathered.is_none() {
        ctx.gathered = Some(ctx.fresh("picks"));
    }
    ctx.gathered.clone()
}

/// Every zone a pick of [`add_chosen`] can be in by the time a later line reads it.
fn chosen_zones() -> Value {
    let zones = [
        "Battlefield",
        "Hand",
        "Library",
        "Graveyard",
        "Exile",
        "Stack",
    ]
    .map(|zone| json!({ "In": zone }));
    json!({ "AnyOf": zones })
}

/// `ChooseCard`: the chooser picks cards that match, optionally or at random; the later lines read them as `ChosenCard`.
#[allow(clippy::too_many_lines)]
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
            "DefinedCards",
            "TargetControls",
            "Optional",
            "ChooseEach",
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
        // "Each player chooses a creature they control": each chooser among their own; a card in a hand or a graveyard is
        // its owner's. A random pick has no chooser.
        Some("Chooser") if p.get("AtRandom") != Some("True") => {
            let whose = match p.get("ChoiceZone") {
                None | Some("Battlefield") => "ControlledBy",
                Some("Hand" | "Graveyard") => "OwnedBy",
                Some(_) => return unmapped("key:ChooseCard.ControlledByPlayer=Chooser-zone"),
            };
            filter.push(json!({ whose: "Chooser" }));
        }
        Some(other) => return unmapped(format!("key:ChooseCard.ControlledByPlayer={other}")),
        None => {}
    }
    // "That player chooses up to two creatures they control": the pool is the chooser's own.
    if p.get("TargetControls") == Some("True") && p.get("AtRandom") != Some("True") {
        let player = match &chooser {
            Some(target) => target.clone(),
            None => defined(ctx, p.get("Defined").unwrap_or("You"))?,
        };
        filter.push(json!({"ControlledBy": player}));
    }
    if let Some(each) = p.get("ChooseEach") {
        return choose_each(ctx, p, each, &filter, chooser);
    }
    // "Choose three target permanents ... one of them at random": the pool is the targets themselves.
    let named = match p.get("DefinedCards") {
        None => None,
        Some("Targeted") if p.get("AtRandom") == Some("True") => Some(ctx.last_target()?),
        Some(_) => return unmapped("key:ChooseCard.DefinedCards"),
    };
    let from = named.unwrap_or_else(|| json!({"Each": filter}));
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
    let before = ctx.chosen_card.replace(bind.clone());
    let mut effects = vec![effect];
    if p.get("RememberChosen") == Some("True")
        && let Some(into) = gathers(ctx, p)
    {
        // "For each player, choose a card ...": the pick of each pass joins the picks of the passes before.
        effects.push(json!({"Bind": {"what": {"Bound": bind}, "as": into, "adds": true}}));
        ctx.remembered = Some(json!({ "Bound": into }));
    } else if p.get("RememberChosen") == Some("True") {
        ctx.remembered = Some(match ctx.remembered.take() {
            None => json!({ "Bound": bind }),
            Some(held) => add_chosen(ctx, &held, before.as_deref(), &bind)?,
        });
    }
    // A title that says "to reveal" is the Oracle wording of a choice shown to the table, with or without `Reveal$`.
    let titled = p
        .get("ChoiceTitle")
        .is_some_and(|title| title.to_lowercase().contains(" to reveal"));
    if p.get("Reveal") == Some("True") || titled {
        effects.push(json!({"Reveal": {"what": {"Bound": bind}}}));
    }
    Ok(effects)
}

/// `ChooseEach$ Artifact & Creature & Land`: the chooser picks one card of each kind, a card never counting for two kinds;
/// the later lines read all the picks as the chosen cards.
fn choose_each(
    ctx: &mut Ctx,
    p: &Params,
    each: &str,
    filter: &[Value],
    chooser: Option<Value>,
) -> Tr<Vec<Value>> {
    if p.get("AtRandom") == Some("True") || !matches!(p.get("Amount"), None | Some("1")) {
        return unmapped("key:ChooseCard.ChooseEach-shape");
    }
    // A party is up to one creature of each party type, not one creature of any of them.
    if each.split(" & ").any(|kind| kind.trim() == "Party") {
        return unmapped("valid-base:Party");
    }
    let min = match (p.get("MinAmount"), p.get("Mandatory")) {
        (Some("0"), _) | (None, None | Some("False")) => 0,
        (None | Some("1"), Some("True")) => 1,
        _ => return unmapped("key:ChooseCard.ChooseEach-shape"),
    };
    let who = match chooser {
        Some(target) => target,
        None => defined(ctx, p.get("Defined").unwrap_or("You"))?,
    };
    let picks = ctx.fresh("picks");
    let mut effects = Vec::new();
    let mut earlier: Vec<Value> = Vec::new();
    for kind in each.split(" & ") {
        let mut pool = filter.to_vec();
        pool.extend(selector_any(ctx, kind)?);
        pool.extend(earlier.iter().map(|name| json!({"Not": {"Is": name}})));
        let bind = ctx.fresh("card");
        effects.push(json!({"Choose": {
            "who": who, "from": {"Each": pool}, "count": {"min": min, "max": 1}, "as": bind}}));
        earlier.push(json!({ "Bound": bind }));
    }
    let any: Vec<Value> = earlier.iter().map(|name| json!({"Is": name})).collect();
    let union = json!({"Each": [{"AnyOf": any}, chosen_zones()]});
    // "For each player, you choose ...": the picks of each pass join the picks of the passes before.
    if p.get("RememberChosen") == Some("True")
        && let Some(into) = gathers(ctx, p)
    {
        effects.push(json!({"Bind": {"what": union, "as": into, "adds": true}}));
        if p.get("Reveal") == Some("True") {
            effects.push(json!({"Reveal": {"what": union}}));
        }
        ctx.remembered = Some(json!({ "Bound": into }));
        return Ok(effects);
    }
    effects.push(json!({"Bind": {"what": union, "as": picks}}));
    ctx.chosen_card = Some(picks.clone());
    if p.get("Reveal") == Some("True") {
        effects.push(json!({"Reveal": {"what": {"Bound": picks}}}));
    }
    if p.get("RememberChosen") == Some("True") {
        if ctx.remembered.is_some() || ctx.player_loop.is_some() {
            return unmapped("key:ChooseCard.ChooseEach-remembered");
        }
        ctx.remembered = Some(json!({ "Bound": picks }));
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
            "DefinedCards",
            "Zone",
            "RepeatSubAbility",
            "DamageMap",
            "ChangeZoneTable",
            "UseImprinted",
        ],
    )?;
    // The tables gather the moves and the damage of all passes for the triggers that read them as one batch.
    let together = ["ChangeZoneTable", "DamageMap"]
        .iter()
        .any(|table| p.get(table) == Some("True"));
    let bind = ctx.fresh("each");
    let mut before_move = None;
    let over = match (p.get("RepeatPlayers"), p.get("RepeatCards")) {
        (None, None) if p.has("DefinedCards") => defined_cards(ctx, p, &bind, &mut before_move)?,
        (None, Some(valid)) => {
            // A pass names the object where it was found, so its controller is read from it, as it last was once it moved.
            before_move = Some(json!({ "Bound": bind }));
            json!({"Each": selector_in(ctx, valid, p.get("Zone"))?})
        }
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
    // `UseImprinted$` names the one the pass is for `Imprinted`, so `Remembered` keeps what it held before the loop.
    let held = json!({ "Bound": bind });
    let imprint = p.get("UseImprinted") == Some("True");
    let (outer, outer_imprinted) = (ctx.remembered.clone(), ctx.imprinted.clone());
    if imprint {
        ctx.imprinted = Some(held);
    } else {
        ctx.remembered = Some(held);
    }
    if let Some(before) = before_move {
        ctx.moved_from.insert(bind.clone(), before);
    }
    let part = ctx.sub(sub)?;
    let outer_loop = ctx.player_loop.take();
    if p.has("RepeatPlayers") && !imprint {
        ctx.player_loop = Some(bind.clone());
    }
    let outer_gathered = ctx.gathered.take();
    let effects = chain_at(ctx, &part, false);
    ctx.player_loop = outer_loop;
    ctx.moved_from.remove(&bind);
    let gathered =
        std::mem::replace(&mut ctx.gathered, outer_gathered).map(|name| json!({ "Bound": name }));
    let into = gathered.as_ref().map(|picks| {
        (
            ctx.remembered.as_ref() == Some(picks),
            ctx.imprinted.as_ref() == Some(picks),
        )
    });
    (ctx.remembered, ctx.imprinted) = (outer, outer_imprinted);
    // The picks the passes gathered are what the slot holds after the repeat; one that held something before stays a gap.
    // "Each opponent chooses two cards in their graveyard and exiles the rest": Forge reads "the rest" as every card not
    // picked, the graveyard of a player who picked nothing included, so picks of some players read as the rest are a gap.
    let some_players = p.get("RepeatPlayers") != Some("Player");
    if into.is_some()
        && some_players
        && ctx
            .svars
            .values()
            .any(|text| text.contains("!IsRemembered"))
    {
        return unmapped("key:ChooseCard.RememberChosen-of-some-players-read-as-the-rest");
    }
    match into {
        Some((true, false)) if ctx.remembered.is_none() => ctx.remembered = gathered,
        Some((false, true)) if ctx.imprinted.is_none() => ctx.imprinted = gathered,
        Some(_) => return unmapped("key:ChooseCard.RememberChosen-with-remembered"),
        None => {}
    }
    let repeat = json!({"RepeatEach": {"over": over, "as": bind, "effects": effects?}});
    Ok(vec![if together {
        json!({"Together": {"effects": [repeat]}})
    } else {
        repeat
    }])
}

/// `DefinedCards$` of a `RepeatEach`: the objects an earlier step named, one pass for each. `before_move` is set to how a
/// pass reads the one it is for as it was before the step that moved it (its controller is the last known one, CR 400.7):
/// a target a step moved is still named by its old reference, what a step bound is the object it became.
fn defined_cards(ctx: &Ctx, p: &Params, each: &str, before_move: &mut Option<Value>) -> Tr<Value> {
    let word = p.get("DefinedCards").unwrap_or_default();
    let list = match word {
        "Targeted" => ctx.last_target()?,
        "DirectRemembered" | "Remembered" | "RememberedCard" => {
            super::memory::slot_ref(ctx, super::memory::Slot::Remembered)
                .map_or_else(|| unmapped("defined:Remembered-without-a-change"), Ok)?
        }
        "Imprinted" => super::memory::imprinted(ctx)?,
        other => return unmapped(format!("key:RepeatEach.DefinedCards={other}")),
    };
    let moved = list
        .get("Bound")
        .and_then(Value::as_str)
        .is_some_and(|name| ctx.counted.contains(name));
    if list.get("Target").is_some() {
        *before_move = Some(json!({ "Bound": each }));
    } else if moved {
        *before_move = Some(json!({"Departed": {"Bound": each}}));
    }
    Ok(match p.get("Zone") {
        None => list,
        Some(zone) => json!({"Each": [{"Is": list}, {"In": zone}]}),
    })
}

/// `ImmediateTrigger`: a reflexive trigger that runs the named ability with targets of its own. `RememberObjects$` names
/// the objects the trigger is about, bound now (the trigger has targets of its own, so a `Target` would be another one)
/// and read by `DelayTriggerRemembered` in its steps.
pub fn immediate_trigger(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "ImmediateTrigger",
        &[
            "Execute",
            "UnlessSwitched",
            "RememberObjects",
            "RememberSVarAmount",
        ],
    )?;
    let Some(name) = p.get("Execute") else {
        return unmapped("key:ImmediateTrigger.no-Execute");
    };
    let part = ctx.sub(name)?;
    let mut out = Vec::new();
    let mut bound = None;
    // The reflexive trigger of a sacrifice cost reads the sacrificed object as `Spawner>Sacrificed`.
    let spawned = ctx
        .svars
        .values()
        .any(|text| text.contains("Spawner>Sacrificed"))
        .then_some("Sacrificed");
    if let Some(word) = p.get("RememberObjects").or(spawned) {
        let loose = super::memory::slot_ref_loose(ctx, super::memory::Slot::Remembered)
            .filter(|held| super::memory::is_scoped(ctx, held));
        let what = match word {
            // "You may discard a card. When you do, ...": the trigger names what the step inside the "may" bound, which
            // holds nothing when that step did not run.
            "Remembered" | "RememberedCard" | "RememberedLKI" if loose.is_some() => {
                loose.unwrap_or_default()
            }
            // A target an earlier step moved is another object now (CR 400.7), so `Targeted` stays a gap.
            "ChosenCard" | "ChosenPlayer" | "Sacrificed" => defined(ctx, word)?,
            w if super::triggered::is_word(w) => defined(ctx, w)?,
            w => super::refs::read_or(ctx, w, super::refs::Scope::Reflexive, || {
                format!("key:ImmediateTrigger.RememberObjects={w}")
            })?,
        };
        let bind = ctx.fresh("when");
        out.push(json!({"Bind": {"what": what, "as": bind}}));
        bound = Some(json!({ "Bound": bind }));
    }
    // "that player" of the spawning damage trigger, read by the reflexive trigger's target filters.
    if bound.is_none()
        && part
            .get("ValidTgts")
            .is_some_and(|valid| valid.contains("Spawner>TriggeredTarget"))
        && ctx.trigger.as_ref().is_some_and(|shape| shape.to_player)
    {
        let what = defined(ctx, "TriggeredTarget")?;
        let bind = ctx.fresh("when");
        out.push(json!({"Bind": {"what": what, "as": bind}}));
        bound = Some(json!({ "Bound": bind }));
    }
    // `RememberSVarAmount$`: the number as the trigger is set up, which its steps read as `TriggerRememberAmount`.
    let mut kept = None;
    if let Some(word) = p.get("RememberSVarAmount") {
        let value = amount(ctx, word)?;
        let bind = ctx.fresh("number");
        out.push(json!({"Let": {"as": bind, "amount": value}}));
        kept = Some(bind);
    }
    let outer_amount = std::mem::replace(&mut ctx.trigger_amount, kept);
    let outer_delayed = std::mem::replace(&mut ctx.delayed_remembered, bound);
    let outer = std::mem::take(&mut ctx.targets);
    let parent = ctx.parent_target.take();
    let effects = chain_at(ctx, &part, false);
    let targets = std::mem::replace(&mut ctx.targets, outer);
    ctx.parent_target = parent;
    ctx.delayed_remembered = outer_delayed;
    // The targets are chosen as the trigger goes on the stack, before its resolution holds the kept number.
    if let Some(name) = std::mem::replace(&mut ctx.trigger_amount, outer_amount)
        && Value::Array(targets.clone())
            .to_string()
            .contains(&json!({ "Bound": name }).to_string())
    {
        return unmapped("key:ImmediateTrigger.RememberSVarAmount-in-a-target");
    }
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
            "ValidTgts",
        ],
    )?;
    // Every player named reveals their own top cards; a pick among them is one player's. "Look at the top card of
    // target player's library": the controller looks at the target's library.
    let who = match (p.get("ValidTgts"), p.get("Defined")) {
        (None, None | Some("You")) => json!("You"),
        (None, Some(word)) => defined(ctx, word)?,
        (Some("Player" | "Opponent" | "Player.Opponent"), None)
            if !p.has("RevealValid") && !p.has("TargetMin") && !p.has("TargetMax") =>
        {
            subject(ctx, p, "You", None)?
        }
        (Some(_), _) => return unmapped("key:PeekAndReveal.ValidTgts"),
    };
    if matches!(who.as_str(), Some("EachPlayer" | "Opponents")) && p.has("RevealValid") {
        return unmapped("key:PeekAndReveal.Defined+RevealValid");
    }
    // "Reveal the top card" without a look first: the same cards are seen when all of them are revealed.
    match p.get("NoPeek") {
        None => {}
        Some("True") if !p.has("RevealValid") && p.get("NoReveal").is_none() => {}
        Some(_) => return unmapped("key:PeekAndReveal.NoPeek-with-a-pick"),
    }
    let top = ctx.fresh("top");
    let from_top = json!({ "Bound": top });
    let count = amount(ctx, p.get("PeekAmount").unwrap_or("1"))?;
    let mut look = json!({"LookAtTop": {"count": count, "as": top}});
    if who != json!("You") {
        look["LookAtTop"]["who"] = who;
    }
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
    // "Roll to visit your Attractions" (CR 701.52): the roll of the game's own rules, not a table of the card.
    if p.get("ToVisitYourAttractions") == Some("True") {
        only(p, "RollDice", &["Sides", "ToVisitYourAttractions"])?;
        if p.get("Sides").unwrap_or("6") != "6" {
            return unmapped("key:RollDice.ToVisitYourAttractions-sides");
        }
        return Ok(vec![json!({"RollToVisit": {}})]);
    }
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
    only(
        p,
        api,
        &["Defined", "Type", "OrColors", "InvalidTypes", "ValidTypes"],
    )?;
    let or_colors = match (api, p.get("OrColors")) {
        (_, None) => false,
        ("ChooseColor", Some("True")) => true,
        (_, Some(other)) => return unmapped(format!("key:{api}.OrColors={other}")),
    };
    let kind = match (api, p.get("Type")) {
        // "The color or colors of your choice": a later line must read the whole set.
        ("ChooseColor", None) if or_colors => "Colors",
        ("ChooseColor", None) => "Color",
        ("ChooseType", Some("Creature")) => "CreatureType",
        ("ChooseType", Some("Card")) => "CardType",
        ("ChooseType", Some("Basic Land" | "Land")) => "LandType",
        (_, other) => return unmapped(format!("key:{api}.Type={}", other.unwrap_or(""))),
    };
    let value_kind = e14c_type_list(p, kind)?.unwrap_or_else(|| json!(kind));
    let who = defined(ctx, p.get("Defined").unwrap_or("You"))?;
    let bind = ctx.fresh("chosen");
    ctx.chosen = Some(bind.clone());
    ctx.chosen_kind = Some(kind);
    Ok(vec![
        json!({"ChooseValue": {"who": who, "kind": value_kind, "as": bind}}),
    ])
}

/// The card types a `ChooseType` offers.
const E14C_CARD_TYPES: [&str; 9] = [
    "Artifact",
    "Battle",
    "Creature",
    "Enchantment",
    "Instant",
    "Kindred",
    "Land",
    "Planeswalker",
    "Sorcery",
];

/// A `ChooseType` limited by `ValidTypes$` (only these card types) or `InvalidTypes$` (every card type but these, or a
/// creature type other than these); None when neither is given. A word that is no card type stays a gap.
fn e14c_type_list(p: &Params, kind: &str) -> Tr<Option<Value>> {
    let words = |key: &str| {
        p.get(key)
            .map(|list| list.split(',').map(str::trim).collect::<Vec<_>>())
    };
    let known = |list: &[&str]| list.iter().all(|word| E14C_CARD_TYPES.contains(word));
    match (kind, words("ValidTypes"), words("InvalidTypes")) {
        (_, None, None) => Ok(None),
        ("CardType", Some(valid), None) if valid.len() >= 2 && known(&valid) => {
            Ok(Some(json!({"CardTypeOf": valid})))
        }
        ("CardType", None, Some(invalid)) if known(&invalid) => {
            let left: Vec<&str> = E14C_CARD_TYPES
                .into_iter()
                .filter(|kind| !invalid.contains(kind))
                .collect();
            Ok(Some(json!({"CardTypeOf": left})))
        }
        ("CreatureType", None, Some(invalid)) if !invalid.is_empty() => {
            Ok(Some(json!({"CreatureTypeExcept": invalid})))
        }
        (_, Some(_), _) => unmapped("key:ChooseType.ValidTypes"),
        (_, _, Some(_)) => unmapped("key:ChooseType.InvalidTypes"),
    }
}

/// `ChooseNumber`: one player picks a whole number from `Min$` (0) to `Max$`, read when the choice is made; the later lines
/// read it as `ChosenNumber`. Without a `Max$` the range is open and stays a gap.
pub fn choose_number(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    if p.has("Highest") || p.has("NotLowest") {
        return super::dk27_numbers::dk27_secret_numbers(ctx, p);
    }
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
        (Some("Combat"), Some("EndCombat") | None, None) => {
            json!(["Combat"])
        }
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

/// `Repeat` with a fixed number of passes (`MaxRepeat`): the body again and again, at most that many times. With a check
/// (`RepeatCheckSVar$`, `RepeatPresent$`) the body is done once and then again while the check holds, as Forge's
/// do-while does, still never more than [`MOST_PASSES`] times. `RepeatOptional$` asks the caster before each pass after
/// the first (`ep25_repeat`); `ValidTgts$` is the target the body names.
pub fn repeat(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Repeat",
        &[
            "RepeatSubAbility",
            "MaxRepeat",
            "RepeatCheckSVar",
            "RepeatSVarCompare",
            "RepeatPresent",
            "RepeatDefined",
            "RepeatCompare",
            "RepeatOptional",
            "ValidTgts",
        ],
    )?;
    let Some(sub) = p.get("RepeatSubAbility") else {
        return unmapped("key:Repeat.no-RepeatSubAbility");
    };
    let ask = super::ep25_repeat::asker(p)?;
    super::ep25_repeat::target(ctx, p)?;
    let built = repeat_built(ctx, p, sub)?;
    Ok(super::ep25_repeat::asking(built, ask))
}

/// The `RepeatWhile` of a `Repeat` line whose body is `sub`, before any question to go on.
fn repeat_built(ctx: &mut Ctx, p: &Params, sub: &str) -> Tr<Vec<Value>> {
    let outer = ctx.remembered.clone();
    let part = ctx.sub(sub)?;
    let effects = chain_at(ctx, &part, false);
    // The check is read after the body, which it follows, so it sees what the body remembers.
    let check = effects.is_ok().then(|| repeat_check(ctx, p));
    ctx.remembered = outer;
    let effects = effects?;
    if let Some(check) = check.transpose()?.flatten() {
        // "If you have fewer than seven cards in hand, repeat this process": the check comes before the next pass,
        // which a script written as a do-while does once too often.
        let said = p.get("StackDescription").or(p.get("SpellDescription"));
        if said.is_some_and(|text| text.starts_with("If ")) {
            return unmapped("key:Repeat.check-first");
        }
        return repeat_checked(p, &effects, &check);
    }
    let Some(max) = p.get("MaxRepeat") else {
        if p.has("RepeatOptional") {
            return Ok(vec![super::ep25_repeat::until_stopped(&effects)]);
        }
        return unmapped("key:Repeat.no-MaxRepeat");
    };
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

/// The check that ends a `Repeat`: a count compared (`RepeatCheckSVar$`, at least 1 by default) and a presence
/// (`RepeatPresent$` among `RepeatDefined$`), all of them holding; `None` without either.
fn repeat_check(ctx: &Ctx, p: &Params) -> Tr<Option<Value>> {
    if p.has("RepeatSVarCompare") && !p.has("RepeatCheckSVar") {
        return unmapped("key:Repeat.RepeatSVarCompare");
    }
    if (p.has("RepeatDefined") || p.has("RepeatCompare")) && !p.has("RepeatPresent") {
        return unmapped("key:Repeat.RepeatDefined");
    }
    let renamed: Vec<String> = [
        ("RepeatCheckSVar", "CheckSVar"),
        ("RepeatSVarCompare", "SVarCompare"),
        ("RepeatPresent", "IsPresent"),
        ("RepeatDefined", "PresentDefined"),
        ("RepeatCompare", "PresentCompare"),
    ]
    .iter()
    .filter_map(|(from, to)| p.get(from).map(|value| format!("{to}$ {value}")))
    .collect();
    if renamed.is_empty() {
        return Ok(None);
    }
    super::condition::condition(ctx, &Params::parse(&renamed.join(" | ")))
}

/// A checked repeat: the body once, then again while the check holds, at most `MaxRepeat$` (or [`MOST_PASSES`]) passes.
fn repeat_checked(p: &Params, effects: &[Value], check: &Value) -> Tr<Vec<Value>> {
    let most = match p.get("MaxRepeat").map(str::parse::<u32>) {
        None => MOST_PASSES,
        Some(Ok(n)) if (1..=MOST_PASSES).contains(&n) => n,
        Some(_) => return unmapped("key:Repeat.MaxRepeat-with-a-check"),
    };
    Ok(vec![json!({"RepeatWhile": {
        "condition": check, "effects": effects, "most": most, "at_least_once": true}})])
}
