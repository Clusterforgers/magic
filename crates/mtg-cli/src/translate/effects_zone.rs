//! Zone changes: `ChangeZone` of a target or of the card itself, the library search, and `ChangeZoneAll`.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, check_keys, only, unmapped};
use super::frame::{self, Goes, Parties, Slot, Verb, goes, moves, parent_is_player};
use super::keyword::keyword_list;
use super::remember;
use super::valid::{defined, selector_any, selector_in, subject};
use crate::forge::script::Params;

/// The effects of `ChangeZone` and `ChangeZoneAll`.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Tr<Vec<Value>> {
    if p.has("ExileFaceDown") {
        return sealed_move(ctx, p, api);
    }
    if api == "ChangeZone" && p.get("Origin") == Some("Sideboard") {
        return outside_game(ctx, p);
    }
    let origin = if api == "ChangeZone" && p.get("Origin") == Some("Stack") {
        "Stack"
    } else if api == "ChangeZone"
        && p.get("Origin") == Some("Battlefield,Stack")
        && p.has("ValidTgts")
    {
        // "Target spell or permanent": the target string says which zone each alternative is in (`Card.inZoneStack`).
        "Battlefield"
    } else if api == "ChangeZone"
        && matches!(p.get("Origin"), None | Some("All"))
        && named_objects(p)
    {
        // The named objects are moved from wherever they are now; a zone is no filter on them.
        "All"
    } else {
        zone_word(p.get("Origin"), "Origin", true)?
    };
    let destination = zone_word(p.get("Destination"), "Destination", false)?;
    match (api, origin) {
        ("ChangeZone", "Library") if p.has("Defined") && !p.has("ChangeType") => {
            change_zone(ctx, p, origin, destination)
        }
        ("ChangeZone", "Library") => search(ctx, p, destination),
        ("ChangeZone", from) if searches_library(from) && p.has("ChangeType") => {
            search(ctx, p, destination)
        }
        ("ChangeZone", from)
            if (hidden_origin(from)
                || (from == "Battlefield" && p.get("Hidden") == Some("True")))
                && p.has("ChangeType") =>
        {
            choose_and_move(ctx, p, origin, destination)
        }
        // "Put two cards from your hand on top of your library": the cards are not named, so the player picks.
        ("ChangeZone", from)
            if hidden_origin(from)
                && (p.has("ChangeNum") || p.get("Hidden") == Some("True"))
                && !["ChangeType", "Defined", "ValidTgts"]
                    .iter()
                    .any(|key| p.has(key)) =>
        {
            choose_and_move(ctx, p, origin, destination)
        }
        ("ChangeZone", _) => change_zone(ctx, p, origin, destination),
        _ => change_zone_all(ctx, p, origin, destination),
    }
}

/// `ExileFaceDown$ True` on a `ChangeZone` or `ChangeZoneAll`: the cards go to exile face down and no player may look at
/// them (CR 406.3), their owner and the player who exiled them included. The line is read without the key and each exile
/// it makes is laid face down (`WithMayLook$ True`: for its exiler to look at); an exile that carries more than its objects and their name (a duration), a move to exile
/// by another step, or a line with a look or a foretelling is a gap.
fn sealed_move(ctx: &mut Ctx, p: &Params, api: &str) -> Tr<Vec<Value>> {
    if p.get("ExileFaceDown") != Some("True") || p.get("Destination") != Some("Exile") {
        return unmapped(format!("key:{api}.ExileFaceDown"));
    }
    // `WithMayLook$ True`: the player whose effect it is may look at them.
    let way = match p.get("WithMayLook") {
        None => "sealed",
        Some("True") => "held",
        Some(_) => return unmapped(format!("key:{api}.WithMayLook")),
    };
    for key in [
        "Duration",
        "Foretold",
        "ForetoldCost",
        "StaticEffect",
        "AtEOT",
        "DestinationAlternative",
    ] {
        if p.has(key) {
            return unmapped(format!("key:{api}.ExileFaceDown-with-{key}"));
        }
    }
    let mut effects = effects(ctx, &p.without(&["ExileFaceDown", "WithMayLook"]), api)?;
    let mut sealed = 0;
    for effect in &mut effects {
        seal(effect, way, &mut sealed)?;
    }
    if sealed == 0 {
        return unmapped(format!("key:{api}.ExileFaceDown-shape"));
    }
    // What the line remembers is the cards in exile (CR 400.7), not the ones a search found in the library.
    if p.get("RememberChanged") == Some("True") {
        let unnamed: Vec<&mut Value> = effects
            .iter_mut()
            .filter(|effect| {
                effect
                    .get("ExileHidden")
                    .is_some_and(|e| e.get("as").is_none())
            })
            .collect();
        match unnamed.len() {
            0 if sealed == 1 => {}
            1 if sealed == 1 => {
                if let Some(effect) = unnamed.into_iter().next() {
                    remember::bind_moved(ctx, effect, "ExileHidden", "exiled");
                }
            }
            _ => return unmapped(format!("key:{api}.ExileFaceDown-remembered")),
        }
    }
    Ok(effects)
}

/// Lays each plain exile in `value` face down the `way` an `ExileHidden` names (`sealed` or `held`), counting them in
/// `sealed`.
fn seal(value: &mut Value, way: &str, sealed: &mut usize) -> Tr<()> {
    match value {
        Value::Array(items) => {
            for item in items {
                seal(item, way, sealed)?;
            }
        }
        Value::Object(map) => {
            if let (1, Some(Value::Object(exile))) = (map.len(), map.get("Exile")) {
                if exile.keys().any(|key| key != "what" && key != "as") {
                    return unmapped("key:ExileFaceDown-exile-with-more");
                }
                let mut hidden = exile.clone();
                hidden.insert(way.to_owned(), json!(true));
                *value = json!({ "ExileHidden": hidden });
                *sealed += 1;
                return Ok(());
            }
            if map.get("to").and_then(Value::as_str) == Some("Exile") {
                return unmapped("key:ExileFaceDown-move");
            }
            for item in map.values_mut() {
                seal(item, way, sealed)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// A card from outside the game (`Origin$ Sideboard`): a Commander game has no sideboard, so the search finds nothing. It is
/// read as the library search of the same line, with the zone changed and no shuffle (no library was searched).
fn outside_game(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    let destination = zone_word(p.get("Destination"), "Destination", false)?;
    if destination != "Hand" || p.has("DifferentNames") {
        return unmapped(format!("zone:Origin=Sideboard-to-{destination}"));
    }
    let mut effects = Value::Array(search(ctx, p, destination)?);
    outside(&mut effects, p.get("Reveal") == Some("True"));
    Ok(match effects {
        Value::Array(list) => list,
        other => vec![other],
    })
}

/// Points the searches of a step at the zone outside the game and drops the shuffles.
fn outside(step: &mut Value, reveal: bool) {
    match step {
        Value::Array(list) => {
            list.retain(|item| *item != json!({"Shuffle": {}}));
            for item in list.iter_mut() {
                outside(item, reveal);
            }
        }
        Value::Object(map) => {
            if let Some(search) = map.get_mut("Search") {
                search["zone"] = json!("Outside");
                // Only the card says whether the cards are shown; nothing is found either way.
                search["reveal"] = json!(reveal);
            }
            for item in map.values_mut() {
                outside(item, reveal);
            }
        }
        _ => {}
    }
}

/// Whether the line moves objects it names (`Defined$`), not a target or a pick by type.
fn named_objects(p: &Params) -> bool {
    p.has("Defined") && !p.has("ValidTgts") && !p.has("ChangeType")
}

/// Whether the word is one of the hand, graveyard and exile, or a list of them (`Graveyard,Exile`).
fn hidden_origin(word: &str) -> bool {
    if word == "Command" {
        return true;
    }
    zone_list(word).is_some_and(|zones| {
        zones
            .iter()
            .all(|zone| matches!(*zone, "Hand" | "Graveyard" | "Exile"))
    })
}

/// The zones of a list word (`Graveyard,Hand`), each a known zone once; `None` for any other word (`Stack`, `Command`,
/// `All` have no zone here).
fn zone_list(word: &str) -> Option<Vec<&str>> {
    let zones: Vec<&str> = word.split(',').collect();
    let known = zones.iter().all(|zone| {
        matches!(
            *zone,
            "Battlefield" | "Graveyard" | "Hand" | "Library" | "Exile"
        )
    });
    let distinct = (1..zones.len()).all(|at| !zones[..at].contains(&zones[at]));
    (known && distinct).then_some(zones)
}

/// Whether the word lists the library with the hand and graveyard: a search that may also look there.
fn searches_library(word: &str) -> bool {
    zone_list(word).is_some_and(|zones| {
        zones.len() > 1
            && zones.contains(&"Library")
            && zones
                .iter()
                .all(|zone| matches!(*zone, "Library" | "Graveyard" | "Hand"))
    })
}

/// A zone word of one zone, or of a list of zones when `many` (the origin of a move from either of them).
fn zone_word<'a>(word: Option<&'a str>, key: &str, many: bool) -> Tr<&'a str> {
    match word {
        Some(zone @ ("Battlefield" | "Graveyard" | "Hand" | "Library" | "Exile")) => Ok(zone),
        // The command zone is only ever searched or named, never a destination of this verb.
        Some("Command") if key == "Origin" => Ok("Command"),
        Some(list) if many && zone_list(list).is_some() => Ok(list),
        Some(other) => unmapped(format!("zone:{key}={other}")),
        None => unmapped(format!("key:ChangeZone.no-{key}")),
    }
}

/// `ManifestDread` (CR 701.62a): look at the top two cards of the library, put one of them onto the battlefield face down as
/// a 2/2 creature and the rest into the graveyard. The player picks the card to manifest; the other goes to the graveyard
/// first, so the card kept is the top of the library the manifest takes (a library of one or none still works). Once
/// only: "manifest dread X times" is a gap.
pub fn manifest_dread(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "ManifestDread",
        &["Amount", "DefinedPlayer", "RememberManifested"],
    )?;
    if !matches!(p.get("Amount"), None | Some("1")) {
        return unmapped("key:ManifestDread.Amount");
    }
    let who = defined(ctx, p.get("DefinedPlayer").unwrap_or("You"))?;
    let looked = ctx.fresh("looked");
    let kept = ctx.fresh("kept");
    let rest = ctx.fresh("rest");
    let mut manifest = json!({"Manifest": {"who": who}});
    if p.get("RememberManifested") == Some("True") {
        let name = ctx.fresh("manifested");
        manifest["Manifest"]["as"] = json!(name);
        ctx.remembered = Some(json!({ "Bound": name }));
    }
    Ok(vec![
        json!({"LookAtTop": {"who": who, "count": 2, "as": looked}}),
        json!({"Choose": {"who": who, "from": {"Bound": looked}, "as": kept, "rest_as": rest}}),
        json!({"Move": {"what": {"Bound": rest}, "to": "Graveyard"}}),
        manifest,
    ])
}

/// The card types an `AddType$` word may name; any other word is a supertype or a subtype.
const STATIC_CARD_TYPES: [&str; 8] = [
    "Creature",
    "Artifact",
    "Enchantment",
    "Land",
    "Planeswalker",
    "Instant",
    "Sorcery",
    "Battle",
];

/// The modifications of an `AddType$` list (`Island & Wraith`): a supertype, a card type or a subtype each.
pub(super) fn added_types(list: &str) -> Vec<Value> {
    list.split('&')
        .map(str::trim)
        .filter(|word| !word.is_empty())
        .map(|word| match word {
            "Legendary" | "Snow" | "Basic" => json!({ "AddSupertype": word }),
            _ if STATIC_CARD_TYPES.contains(&word) => json!({ "AddType": word }),
            _ => json!({ "AddSubtype": word }),
        })
        .collect()
}

/// The modifications of the static the `StaticEffect$` `SVar` names ("It's a 1/1 Spirit creature with flying in addition to
/// its other types"): types added, a fixed power and toughness, a color set or added and keywords. A word that takes
/// something away stays a gap.
fn static_effect_mods(ctx: &mut Ctx, name: &str) -> Tr<Vec<Value>> {
    let part = ctx.sub(name)?;
    check_keys(
        &part,
        "ChangeZone.StaticEffect",
        &[
            "Mode",
            "Affected",
            "AddType",
            "SetPower",
            "SetToughness",
            "AddKeyword",
            "SetColor",
            "AddColor",
            "RemoveCardTypes",
        ],
        |_, _| false,
    )?;
    if part.get("Mode") != Some("Continuous") {
        return unmapped("key:ChangeZone.StaticEffect=mode");
    }
    if !matches!(part.get("Affected"), None | Some("Card.IsRemembered")) {
        return unmapped("key:ChangeZone.StaticEffect=affected");
    }
    let mut mods = Vec::new();
    match (part.get("SetPower"), part.get("SetToughness")) {
        (None, None) => {}
        (Some(power), Some(toughness)) => {
            let (Ok(power), Ok(toughness)) = (power.parse::<i32>(), toughness.parse::<i32>())
            else {
                return unmapped("key:ChangeZone.StaticEffect=power");
            };
            mods.push(json!({"SetPT": {"power": power, "toughness": toughness}}));
        }
        _ => return unmapped("key:ChangeZone.StaticEffect=power-alone"),
    }
    let mut types = added_types(part.get("AddType").unwrap_or_default());
    if part.get("RemoveCardTypes") == Some("True") {
        // The card types the line names are the only ones the object has.
        let only: Vec<Value> = types
            .iter()
            .filter_map(|m| m.get("AddType").cloned())
            .collect();
        if only.is_empty() {
            return unmapped("key:ChangeZone.StaticEffect.RemoveCardTypes");
        }
        types.retain(|m| m.get("AddType").is_none());
        types.insert(0, json!({ "SetTypes": only }));
    }
    mods.extend(types);
    if let Some(color) = part.get("SetColor") {
        if !matches!(color, "White" | "Blue" | "Black" | "Red" | "Green") {
            return unmapped("key:ChangeZone.StaticEffect=color");
        }
        mods.push(json!({ "SetColors": [color] }));
    }
    // "A black Zombie in addition to its other colors and types": the color joins the ones it has (layer 5).
    if let Some(color) = part.get("AddColor") {
        if !matches!(color, "White" | "Blue" | "Black" | "Red" | "Green") {
            return unmapped("key:ChangeZone.StaticEffect=color");
        }
        mods.push(json!({ "AddColors": [color] }));
    }
    if let Some(words) = part.get("AddKeyword") {
        for keyword in keyword_list(ctx, words)? {
            mods.push(json!({ "AddKeyword": keyword }));
        }
    }
    if mods.is_empty() {
        return unmapped("key:ChangeZone.StaticEffect=nothing");
    }
    Ok(mods)
}

/// `StaticEffect$` on a move to the battlefield: the moved objects get the modifications of the named static for as long as
/// they stay there (a new object after they leave is not affected, CR 400.7). The move names what it moved.
fn with_static_effect(
    ctx: &mut Ctx,
    p: &Params,
    destination: &str,
    effects: &mut Vec<Value>,
) -> Tr<()> {
    let Some(name) = p.get("StaticEffect") else {
        return Ok(());
    };
    if destination != "Battlefield" {
        return unmapped("key:ChangeZone.StaticEffect=destination");
    }
    let mods = static_effect_mods(ctx, name)?;
    let moved = frame::moved_name(
        ctx,
        effects,
        "Move",
        "moved",
        "key:ChangeZone.StaticEffect=move",
    )?;
    effects.push(json!({"Continuous": {
        "affected": {"Bound": moved},
        "mods": mods,
        "duration": "Forever",
    }}));
    Ok(())
}

/// `AtEOT$` on a move to the battlefield ("Sacrifice it at the beginning of the next end step"): a delayed trigger that
/// acts on exactly the objects the move put there, as the objects they became (CR 400.7).
fn at_end_step(ctx: &mut Ctx, p: &Params, destination: &str, effects: &mut Vec<Value>) -> Tr<()> {
    let Some(word) = p.get("AtEOT") else {
        return Ok(());
    };
    if destination != "Battlefield" || effects.len() != 1 {
        return unmapped("key:ChangeZone.AtEOT");
    }
    let moved = frame::moved_name(ctx, effects, "Move", "moved", "key:ChangeZone.AtEOT")?;
    effects.push(super::tokens_copy::at_eot(
        word,
        &json!({ "Bound": moved }),
    )?);
    Ok(())
}

/// The spell a move from the stack names without a target: the spell itself (`Self`, or `Parent` and nothing on a
/// sorcery or instant, whose chain is the spell), or the spell a cast trigger is about. Any other word is a gap.
fn spell_subject(ctx: &Ctx, p: &Params) -> Tr<Value> {
    let spell = ctx.types.contains("Instant") || ctx.types.contains("Sorcery");
    match p.get("Defined") {
        None | Some("Self") => Ok(json!("This")),
        Some("Parent") if spell => Ok(json!("This")),
        Some("TriggeredSpellAbility" | "TriggeredCard")
            if ctx
                .trigger
                .as_ref()
                .is_some_and(|shape| shape.mode == "SpellCast") =>
        {
            Ok(json!("Triggering"))
        }
        Some(other) => unmapped(format!("defined:Stack-{other}")),
    }
}

/// `ChangeZone` of objects the line names or targets.
const NAMED: Verb = Verb {
    label: "ChangeZone",
    slots: &[
        Slot::Actor,
        Slot::Named,
        Slot::Target,
        Slot::Limits,
        Slot::Count,
        Slot::Decider,
        Slot::Optional,
        Slot::Destination,
    ],
    actor: "DefinedPlayer",
    pool: "",
    count: "ChangeNum",
    own: &[
        "RememberChanged",
        "RememberLKI",
        "RememberTargets",
        "StaticEffect",
        "Duration",
        "Hidden",
        "ThisDefinedAndTgts",
        "AtEOT",
        "ShuffleNonMandatory",
    ],
};

/// `ChangeZone` of cards a player picks from a public zone or a hand.
const PICKED: Verb = Verb {
    label: "ChangeZone.Choose",
    slots: &[
        Slot::Actor,
        Slot::Pool,
        Slot::Target,
        Slot::Limits,
        Slot::Count,
        Slot::Decider,
        Slot::Optional,
        Slot::Destination,
    ],
    actor: "DefinedPlayer",
    pool: "ChangeType",
    count: "ChangeNum",
    own: &[
        "Hidden",
        "AtRandom",
        "RememberChanged",
        "Reorder",
        "StaticEffect",
        "Duration",
    ],
};

/// `ChangeZone` of cards a player finds in a library.
const SEARCHED: Verb = Verb {
    label: "ChangeZone.Search",
    slots: &[
        Slot::Actor,
        Slot::Pool,
        Slot::Target,
        Slot::Limits,
        Slot::Count,
        Slot::Decider,
        Slot::Optional,
        Slot::Destination,
    ],
    actor: "DefinedPlayer",
    pool: "ChangeType",
    count: "ChangeNum",
    own: &[
        "Hidden",
        "ShuffleNonMandatory",
        "RememberChanged",
        "Reveal",
        "OriginAlternative",
        "NoLooking",
        "Reorder",
        "DifferentNames",
        "NoShuffle",
    ],
};

/// The objects a line moves: what it targets or names, and the source too with `ThisDefinedAndTgts$ Self`. A spell on the
/// stack is named without a target.
fn moved_objects(ctx: &mut Ctx, p: &Params, origin: &str) -> Tr<Vec<Value>> {
    if origin == "Stack" && !p.has("ValidTgts") {
        return Ok(vec![spell_subject(ctx, p)?]);
    }
    let what = frame::affects(ctx, p, "Self", Some(origin))?;
    match p.get("ThisDefinedAndTgts") {
        None => Ok(vec![what]),
        Some("Self") if p.has("ValidTgts") && !p.has("Optional") => Ok(vec![json!("This"), what]),
        Some(_) => unmapped("key:ChangeZone.ThisDefinedAndTgts"),
    }
}

fn change_zone(ctx: &mut Ctx, p: &Params, origin: &str, destination: &str) -> Tr<Vec<Value>> {
    NAMED.admit(p)?;
    // "You may shuffle up to four target cards into your library": no shuffle when nothing was chosen, which is what a
    // shuffle-in of no cards does.
    if p.has("ShuffleNonMandatory")
        && (p.get("ShuffleNonMandatory") != Some("True")
            || destination != "Library"
            || p.get("Shuffle") != Some("True")
            || p.get("TargetMin") != Some("0"))
    {
        return unmapped("key:ChangeZone.ShuffleNonMandatory");
    }
    // A target is one object: `ChangeNum$ 1` says so again; any other count, or one beside target bounds, is not read.
    if p.has("ChangeNum")
        && (p.get("ChangeNum") != Some("1")
            || !p.has("ValidTgts")
            || p.has("TargetMax")
            || p.has("TargetMin"))
    {
        return unmapped("key:ChangeZone.ChangeNum");
    }
    // `Mandatory$ True` is the default of a target; a line that is also optional is a contradiction.
    if p.has("Mandatory") && (p.get("Mandatory") != Some("True") || p.has("Optional")) {
        return unmapped("key:ChangeZone.Mandatory");
    }
    // A hidden origin with the objects named is the same move; without them a player would pick, which this shape does not say.
    if p.has("Hidden") && (p.get("Hidden") != Some("True") || !p.has("Defined")) {
        return unmapped("key:ChangeZone.Hidden");
    }
    // What a later line reads of a target named again is its past (last known information), which no name here holds.
    if p.get("Defined") == Some("Targeted")
        && p.has("ValidTgts")
        && remember::wanted(p, &["RememberChanged", "RememberLKI", "RememberTargets"])
    {
        return unmapped("key:ChangeZone.Defined-with-memory");
    }
    let whats = moved_objects(ctx, p, origin)?;
    let two_ways = p.has("DestinationAlternative");
    let keeps = p.has("Duration")
        || p.has("StaticEffect")
        || remember::wanted(p, &["RememberChanged", "RememberLKI", "RememberTargets"]);
    if (two_ways && (keeps || whats.len() > 1)) || (keeps && whats.len() > 1) {
        return unmapped("key:ChangeZone.DestinationAlternative-with-memory");
    }
    let mut effects = Vec::new();
    for what in &whats {
        let places = frame::places(ctx, p, destination, false)?;
        effects.extend(frame::place(ctx, what, places));
    }
    if let Some(effect) = effects.first_mut().filter(|_| !two_ways) {
        keep_track(ctx, p, effect, origin, destination)?;
    }
    if !two_ways {
        with_static_effect(ctx, p, destination, &mut effects)?;
        at_end_step(ctx, p, destination, &mut effects)?;
    } else if p.has("AtEOT") {
        return unmapped("key:ChangeZone.AtEOT-with-two-ways");
    }
    enters_as_said(ctx, p, destination, &mut effects)?;
    if !frame::is_optional(p) {
        return Ok(effects);
    }
    let who = frame::decider(ctx, p, &NAMED)?;
    if who != json!("You") && keeps {
        return unmapped("key:ChangeZone.Optional-remembered");
    }
    // One question stands for all the objects: only you may answer for several targets at once.
    if who != json!("You") && p.has("TargetMax") {
        return unmapped("key:ChangeZone.Optional-for-several");
    }
    frame::optional(p, NAMED.label, effects, &who)
}

/// What the destination adds after the move: joining the attack, the counters of an exile.
fn enters_as_said(
    ctx: &mut Ctx,
    p: &Params,
    destination: &str,
    effects: &mut Vec<Value>,
) -> Tr<()> {
    frame::enters_attacking(ctx, p, effects)?;
    if destination == "Exile" {
        frame::exiled_with_counters(ctx, p, effects)?;
    }
    Ok(())
}

/// `Duration$` and `Remember*` of a move: how long it lasts and the name of what it moved.
fn keep_track(
    ctx: &mut Ctx,
    p: &Params,
    effect: &mut Value,
    origin: &str,
    destination: &str,
) -> Tr<()> {
    match p.get("Duration") {
        None => {}
        // "Exile it until ~ leaves the battlefield": the exiled permanent returns when the source goes.
        Some("UntilHostLeavesPlay") if destination == "Exile" && origin == "Battlefield" => {
            effect["Exile"]["duration"] = json!("WhileSourceOnBattlefield");
        }
        Some(_) => return unmapped("key:ChangeZone.Duration"),
    }
    if remember::wanted(p, &["RememberChanged", "RememberLKI", "RememberTargets"]) {
        bind_moved(ctx, effect);
    }
    Ok(())
}

/// Names what the step moved and makes it what `Remembered` stands for afterwards.
fn bind_moved(ctx: &mut Ctx, moved: &mut Value) {
    let (verb, stem) = if moved.get("Exile").is_some() {
        ("Exile", "exiled")
    } else {
        ("Move", "moved")
    };
    remember::bind_moved(ctx, moved, verb, stem);
}

/// The pool of a pick from the battlefield and whose permanents it holds. The line's own words may say whose; a named
/// player (`DefinedPlayer$`) picks among what they control; without one the pick is among everyone's.
fn battlefield_owner(p: &Params, who: &Parties, filter: &mut Vec<Value>) -> Tr<()> {
    let says_whose = filter
        .iter()
        .any(|pred| pred.get("ControlledBy").is_some() || pred.get("OwnedBy").is_some());
    if who.targeted && !p.has("DefinedPlayer") && !says_whose {
        return unmapped("key:ChangeZone.Choose.Battlefield-owner");
    }
    let controlled = json!({"ControlledBy": who.acts});
    if p.has("DefinedPlayer")
        && (who.acts != json!("You") || !says_whose)
        && !filter.contains(&controlled)
    {
        filter.push(controlled);
    }
    Ok(())
}

/// Whether the card's text lets the player choose nothing ("may", "up to", "any number").
fn allows_none(ctx: &Ctx, p: &Params) -> bool {
    let text = p
        .get("SpellDescription")
        .unwrap_or(ctx.oracle)
        .to_lowercase();
    text.contains(" may ") || text.contains("up to") || text.contains("any number")
}

/// Whether the card's text takes the card from any graveyard ("from a graveyard") and never from "your graveyard".
fn from_any_graveyard(ctx: &Ctx, p: &Params) -> bool {
    let text = p
        .get("SpellDescription")
        .or_else(|| p.get("StackDescription"))
        .unwrap_or(ctx.oracle)
        .to_lowercase();
    (text.contains("from a graveyard") || text.contains("from any graveyard"))
        && !text.contains("your graveyard")
}

/// Whether the card's text takes a card from exile ("an exiled card", "a card in exile") without saying whose ("you
/// own", "that player owns"): exile is shared, so it is any player's card.
fn from_any_exile(ctx: &Ctx, p: &Params) -> bool {
    let text = p
        .get("SpellDescription")
        .or_else(|| p.get("StackDescription"))
        .unwrap_or(ctx.oracle)
        .to_lowercase();
    ["exiled card", "in exile", "from exile"]
        .iter()
        .any(|words| text.contains(words))
        && !["you own", "they own", " owns ", "your exile"]
            .iter()
            .any(|words| text.contains(words))
}

/// A card of the player's own hand, graveyard or exile, or a permanent, that a player picks and moves ("put a creature card
/// from your hand onto the battlefield"): a choice among the matching cards, then the move.
fn choose_and_move(ctx: &mut Ctx, p: &Params, origin: &str, destination: &str) -> Tr<Vec<Value>> {
    PICKED.admit(p)?;
    // Several cards put into one library are ordered by their controller anyway.
    if p.get("Reorder").is_some_and(|word| word != "True") {
        return unmapped("key:ChangeZone.Choose.Reorder");
    }
    let who = frame::parties(ctx, p, &PICKED)?;
    let (owner, chooser) = (who.acts.clone(), who.decides.clone());
    let mut filter = selector_in(ctx, p.get("ChangeType").unwrap_or("Card"), Some(origin))?;
    if origin == "Battlefield" {
        battlefield_owner(p, &who, &mut filter)?;
    } else if filter.iter().any(|pred| pred.get("OwnedBy").is_some()) {
        if owner != json!("You") {
            return unmapped("key:ChangeZone.Choose.DefinedPlayer");
        }
    } else if origin == "Exile" && filter.iter().any(|pred| pred.get("ExiledWith").is_some()) {
        // Exile is shared: a card exiled with this is named by that, whoever owns it.
    } else if origin == "Exile" && !p.has("DefinedPlayer") && from_any_exile(ctx, p) {
        // "an exiled card named ~": any card in exile when the text names no owner.
    } else if origin == "Graveyard" && !p.has("DefinedPlayer") && from_any_graveyard(ctx, p) {
        // "a creature card from a graveyard": Forge picks among every graveyard when no player is defined.
    } else {
        filter.push(json!({"OwnedBy": owner}));
    }
    let mut count = frame::count(ctx, p, &PICKED)?;
    // "Target player puts a card from their hand on top of their library": a chosen single card is not optional unless the
    // card says "may" or "up to" (Forge leaves the minimum off); the engine clamps it to the cards there are.
    if origin == "Hand"
        && count.is_one
        && !frame::is_optional(p)
        && !p.has("Mandatory")
        && !allows_none(ctx, p)
    {
        count.min = json!(1);
    }
    let bind = ctx.fresh("picked");
    let chosen = json!({ "Bound": bind });
    let choose = if p.get("AtRandom") == Some("True") {
        if frame::is_optional(p) {
            return unmapped("key:ChangeZone.Choose.AtRandom-optional");
        }
        json!({"ChooseAtRandom": {"from": {"Each": filter}, "count": count.max, "as": bind}})
    } else {
        json!({"Choose": {"who": chooser, "from": {"Each": filter}, "count": {"min": count.min, "max": count.max}, "as": bind}})
    };
    let remembers = remember::wanted(p, &["RememberChanged"]);
    let places = frame::places(ctx, p, destination, false)?;
    if places.second.is_some() && (remembers || p.has("StaticEffect") || who.each.is_some()) {
        return unmapped("key:ChangeZone.Choose.DestinationAlternative-with-memory");
    }
    let mut moved = frame::place(ctx, &chosen, places);
    match p.get("Duration") {
        None => {}
        // "Exile any number of creatures you control until ~ leaves the battlefield": each returns when the source goes.
        Some("UntilHostLeavesPlay")
            if origin == "Battlefield"
                && who.each.is_none()
                && moved.len() == 1
                && moved[0].get("Exile").is_some() =>
        {
            moved[0]["Exile"]["duration"] = json!("WhileSourceOnBattlefield");
        }
        Some(_) => return unmapped("key:ChangeZone.Choose.Duration"),
    }
    if remembers && let Some(step) = moved.first_mut() {
        bind_moved(ctx, step);
    }
    if let Some((over, name)) = who.each {
        if remembers || p.has("StaticEffect") {
            return unmapped("key:ChangeZone.Choose.RememberChanged-for-each-player");
        }
        if p.has("Attacking") || (destination == "Exile" && p.has("WithCountersType")) {
            return unmapped("key:ChangeZone.Choose.Attacking-for-each-player");
        }
        let mut effects = vec![choose];
        effects.extend(moved);
        Ok(vec![
            json!({"RepeatEach": {"over": over, "as": name, "effects": effects}}),
        ])
    } else {
        let mut steps = vec![choose];
        steps.extend(moved);
        with_static_effect(ctx, p, destination, &mut steps)?;
        enters_as_said(ctx, p, destination, &mut steps)?;
        Ok(steps)
    }
}

/// The library search: the search, the move of what was found, then the shuffle. The player who decides searches and picks;
/// the library is the actor's.
#[allow(clippy::too_many_lines)]
fn search(ctx: &mut Ctx, p: &Params, destination: &str) -> Tr<Vec<Value>> {
    let had_memory = ctx.remembered.is_some();
    SEARCHED.admit(p)?;
    let seat = frame::parties(ctx, p, &SEARCHED)?;
    let another = seat.acts != seat.decides;
    // Not looking at the library only matters when the cards are not named already.
    if p.has("NoLooking")
        && !p
            .get("ChangeType")
            .is_some_and(|t| t.contains("IsRemembered"))
    {
        return unmapped("key:ChangeZone.Search.NoLooking");
    }
    if p.get("Reorder").is_some_and(|word| word != "True") {
        return unmapped("key:ChangeZone.Search.Reorder");
    }
    // "Permanent" in one alternative only (`Permanent.Snow,Legendary,Saga`) would be dropped from it: refuse the mixture
    // before the selector names another missing word.
    super::valid::permanent_card(p.get("ChangeType").unwrap_or("Card"))?;
    let mut filter = selector_any(ctx, p.get("ChangeType").unwrap_or("Card"))?;
    // A card in a library is a permanent card when it has a permanent type (a Kindred sorcery is not one).
    super::valid::fix9_permanent_card(p.get("ChangeType").unwrap_or("Card"), &mut filter)?;
    let filter_is_open = filter.is_empty();
    let bind = ctx.fresh("found");
    let mut find = json!({"filter": filter, "as": bind});
    if seat.decides != json!("You") {
        find["who"] = seat.decides.clone();
    }
    if another {
        find["of"] = seat.acts.clone();
    }
    let alternative = alternative_zones(p, seat.acts != json!("You") || another)?;
    if let Some(also) = &alternative {
        find["also"] = json!(also);
    }
    let count = frame::count(ctx, p, &SEARCHED)?;
    // Only cards of a stated quality may stay hidden (CR 701.23b): "search your library for three cards" finds three.
    if !count.is_one {
        let min = if filter_is_open { count.min } else { json!(0) };
        find["count"] = json!({"min": min, "max": count.max});
    }
    match p.get("DifferentNames") {
        None => {}
        Some("True") => find["distinct"] = json!(true),
        Some(_) => return unmapped("key:ChangeZone.Search.DifferentNames"),
    }
    let revealed_by_words = p.get("Reorder").is_some() && says_reveal(p);
    if p.get("Reveal") == Some("True")
        || (matches!(destination, "Hand" | "Library") && !filter_is_open)
        || revealed_by_words
    {
        find["reveal"] = json!(true);
    }
    let found = json!({ "Bound": bind });
    if p.get("RememberChanged") == Some("True") {
        if seat.each.is_some() {
            return unmapped("key:ChangeZone.Search.RememberChanged-for-each-player");
        }
        ctx.remembered = Some(found.clone());
    }
    let shuffles = match (p.get("NoShuffle"), p.get("Shuffle")) {
        (None, shuffle) => shuffle,
        (Some("True"), None) if destination != "Library" => Some("False"),
        _ => return unmapped("key:ChangeZone.Search.NoShuffle"),
    };
    // A second remembering search would make `Remembered` the later search alone, but the shuffle asks about both.
    if p.has("NoShuffle") && p.get("RememberChanged") == Some("True") && had_memory {
        return unmapped("key:ChangeZone.Search.NoShuffle-twice");
    }
    let search = json!({ "Search": find });
    // "Search, reveal, then do something with those cards": the cards stay in the library and the steps after read them.
    let stays = destination == "Library"
        && alternative.is_none()
        && !p.has("LibraryPosition")
        && shuffles == Some("False")
        && p.get("RememberChanged") == Some("True");
    if stays {
        return Ok(vec![search]);
    }
    let places = frame::places(ctx, p, destination, true)?;
    if places.second.is_some() && p.get("RememberChanged") == Some("True") {
        return unmapped("key:ChangeZone.Search.DestinationAlternative-with-memory");
    }
    let put = frame::place(ctx, &found, places);
    // Searching only another zone leaves the library as it was: "if you search your library this way, shuffle".
    let shuffle = if alternative.is_some() {
        json!({"If": {"condition": {"Searched": {"zone": "Library"}}, "then": [{"Shuffle": {}}]}})
    } else {
        json!({"Shuffle": {}})
    };
    let shuffle = if seat.acts == json!("You") {
        shuffle
    } else {
        json!({"Shuffle": {"who": seat.acts.clone()}})
    };
    let mut effects = vec![search];
    match (destination, shuffles) {
        // The shuffle comes first when the card goes back on top: "then shuffle and put that card on top".
        ("Library", _) => {
            effects.push(shuffle);
            effects.extend(put);
        }
        (_, Some("False")) => effects.extend(put),
        _ => {
            effects.extend(put);
            effects.push(shuffle);
        }
    }
    let effects = frame::optional(p, NAMED.label, effects, &seat.decides)?;
    Ok(match seat.each {
        Some((over, name)) => {
            vec![json!({"RepeatEach": {"over": over, "as": name, "effects": effects}})]
        }
        None => effects,
    })
}

/// Whether the printed description of a search says the cards are revealed.
fn says_reveal(p: &Params) -> bool {
    p.get("SpellDescription")
        .is_some_and(|text| text.to_lowercase().contains("reveal"))
}

/// The other zones of the searcher a search looks in besides the library, from `OriginAlternative$` or an `Origin$` list.
fn alternative_zones(p: &Params, another: bool) -> Tr<Option<Vec<String>>> {
    // `Origin$ Library,Graveyard` searches the same zones as `OriginAlternative$ Graveyard`; both together are two lists.
    let listed: String = p
        .get("Origin")
        .filter(|word| word.contains(','))
        .map(|word| {
            word.split(',')
                .filter(|zone| *zone != "Library")
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    if another && (p.has("OriginAlternative") || !listed.is_empty()) {
        return unmapped("key:ChangeZone.Search.OriginAlternative-for-another-player");
    }
    if !listed.is_empty() && p.has("OriginAlternative") {
        return unmapped("key:ChangeZone.Search.OriginAlternative");
    }
    let Some(list) = p
        .get("OriginAlternative")
        .or((!listed.is_empty()).then_some(listed.as_str()))
    else {
        return Ok(None);
    };
    let also: Vec<&str> = list.split(',').collect();
    if !also
        .iter()
        .all(|zone| matches!(*zone, "Graveyard" | "Hand"))
        || (1..also.len()).any(|at| also[..at].contains(&also[at]))
    {
        return unmapped("key:ChangeZone.Search.OriginAlternative");
    }
    Ok(Some(also.into_iter().map(str::to_owned).collect()))
}

/// The player whose cards a `ChangeZoneAll` takes (`Defined$`): a player word, the player a parent line targeted, or the
/// controller or owner of its target.
fn zone_owner(ctx: &Ctx, word: &str) -> Tr<Value> {
    match word {
        "Targeted" | "TargetedPlayer" => ctx.player_target(),
        "ParentTarget" if parent_is_player(ctx) => ctx.parent(),
        "You"
        | "Opponent"
        | "Player"
        | "Player.Opponent"
        | "Player.Other"
        | "TriggeredCardController"
        | "TriggeredCardOwner"
        | "TriggeredDefendingPlayer"
        | "TriggeredPlayer"
        | "TargetedController"
        | "TargetedOwner" => defined(ctx, word),
        _ => unmapped("key:ChangeZoneAll.Defined"),
    }
}

/// The keys of a `Dig` line that are read.
const DIG_KEYS: [&str; 22] = [
    "DigNum",
    "RememberRevealed",
    "ChangeNum",
    "ChangeValid",
    "Optional",
    "Reveal",
    "DestinationZone",
    "DestinationZone2",
    "LibraryPosition",
    "LibraryPosition2",
    "RestRandomOrder",
    "SkipReorder",
    "Defined",
    "Tapped",
    "NoReveal",
    "ForceRevealToController",
    "ValidTgts",
    "TargetMin",
    "TargetMax",
    "ExileFaceDown",
    "WithMayLook",
    "RememberChanged",
];

/// Whether the sentence of the Oracle text that looks at the top cards lets the player take none ("you may", "up to", "any
/// number"). Taking one of the cards looked at is not a search, so a sentence that says none of these takes exactly as many
/// as there are (CR 701.20); a text without such a sentence keeps the choice open.
fn sentence_allows_none(oracle: &str) -> bool {
    let text = oracle.replace("\\n", "\n").to_lowercase();
    let Some(start) = ["look at the top", "reveal the top"]
        .iter()
        .filter_map(|opening| text.find(opening))
        .min()
    else {
        return true;
    };
    // Both openings are ASCII and at least 14 bytes long; the opening itself ("reveal the top") takes no card.
    let line = text[start + 14..].split('\n').next().unwrap_or_default();
    // The first sentence that takes a card says whether the player may take none.
    for sentence in line.split(". ").take(3) {
        if ["may", "up to", "any number"]
            .iter()
            .any(|word| sentence.contains(word))
        {
            return true;
        }
        if ["exile", "put ", "reveal", "take", "return"]
            .iter()
            .any(|word| sentence.contains(word))
        {
            return false;
        }
    }
    true
}

/// Whether the Oracle text reveals the card taken from among the cards looked at ("You may reveal a creature card from
/// among them and put it into your hand").
fn r5a_reveals_taken(oracle: &str) -> bool {
    let text = oracle.replace("\\n", "\n").to_lowercase();
    let Some(start) = text.find("look at the top") else {
        return false;
    };
    let line = text[start..].split('\n').next().unwrap_or_default();
    line.split(". ")
        .skip(1)
        .take(2)
        .any(|sentence| sentence.contains("reveal ") && sentence.contains("from among"))
}

/// `Dig`: look at the top cards, take some, put the rest somewhere ("look at the top four cards of your library; you may
/// reveal a creature card from among them and put it into your hand; put the rest on the bottom in any order").
pub fn dig(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "Dig", &DIG_KEYS)?;
    // "The cards revealed this way": every card dug, as each is after its move.
    let revealing = p.get("RememberRevealed") == Some("True");
    if revealing
        && (p.get("Reveal") != Some("True")
            || p.get("RememberChanged") == Some("True")
            || face_down(p)? != Hide::No)
    {
        return unmapped("key:Dig.RememberRevealed-shape");
    }
    if p.get("NoReveal") == Some("True") && p.get("Reveal") == Some("True") {
        return unmapped("key:Dig.NoReveal-with-Reveal");
    }
    let hide = face_down(p)?;
    let hidden = hide != Hide::No;
    let who = subject(ctx, p, "You", None)?;
    let count = amount(ctx, p.get("DigNum").unwrap_or("1"))?;
    // "Exile the top card of your library face down": nobody looks at it, the exiler neither.
    if p.get("ChangeNum") == Some("All")
        && hide != Hide::Held
        && p.get("DestinationZone") == Some("Exile")
        && !p.has("ChangeValid")
    {
        return Ok(vec![exile_top(ctx, p, &who, &count, hide)]);
    }
    let top = ctx.fresh("top");
    let kept = ctx.fresh("kept");
    let rest = ctx.fresh("rest");
    let from_top = json!({ "Bound": top });
    let take = Take::of(ctx, p)?;
    let filter = p.get("ChangeValid");
    let may =
        p.get("Optional") == Some("True") || (filter.is_some() && sentence_allows_none(ctx.oracle));
    // "Put all of them" has no choice: what matches moves, and a "may" would be all or nothing.
    if matches!(take, Take::All) && p.get("Optional") == Some("True") {
        return unmapped("key:Dig.ChangeNum");
    }
    let matching = match filter {
        Some(valid) => {
            let mut selector = vec![json!({"In": "Library"}), json!({"Is": from_top})];
            selector.extend(selector_any(ctx, valid)?);
            selector.extend(super::valid::permanent_card(valid)?);
            json!({"Each": selector})
        }
        None => from_top.clone(),
    };
    let mut choose = match filter {
        Some(_) => json!({"from": matching, "as": kept}),
        None => json!({"from": from_top, "as": kept, "rest_as": rest}),
    };
    match take {
        Take::Count(taken) if may || taken != 1 => {
            let min = if may { 0 } else { taken };
            choose["count"] = json!({"min": min, "max": taken});
        }
        Take::Any => choose["count"] = json!({"min": 0, "max": null}),
        Take::Counted(ref taken) => {
            let min = if may { json!(0) } else { taken.clone() };
            choose["count"] = json!({"min": min, "max": taken});
        }
        Take::Count(_) | Take::All => {}
    }
    let (to, rest_to) = dig_places(p)?;
    let to = match hide {
        Hide::No => to,
        Hide::Held => Goes::Hidden,
        Hide::Sealed => Goes::Sealed,
    };
    let rest_ref = if filter.is_some() {
        from_top.clone()
    } else {
        json!({ "Bound": rest })
    };
    if p.get("RememberChanged") == Some("True") && !matches!(take, Take::All) && !hidden {
        ctx.remembered = Some(json!({ "Bound": kept }));
    }
    let mut effects = vec![json!({"LookAtTop": {"who": who, "count": count, "as": top}})];
    let moved = if matches!(take, Take::All) {
        matching
    } else {
        effects.push(json!({ "Choose": choose }));
        json!({ "Bound": kept })
    };
    // "You may reveal a creature card from among them and put it into your hand": the card taken is shown.
    if filter.is_some()
        && !hidden
        && matches!(&to, Goes::To(zone) if *zone == json!("Hand"))
        && p.get("Reveal") != Some("True")
        && p.get("NoReveal") != Some("True")
        && r5a_reveals_taken(ctx.oracle)
    {
        effects.push(json!({"Reveal": {"what": moved.clone()}}));
    }
    if p.get("Reveal") == Some("True") {
        // "Reveal the top X cards" shows every card dug, not only the ones taken.
        effects.push(json!({"Reveal": {"what": from_top}}));
    }
    let mut put = moves(&moved, to);
    if (hidden || matches!(take, Take::All)) && p.get("RememberChanged") == Some("True") {
        let (verb, stem) = if hidden {
            ("ExileHidden", "exiled")
        } else if put.get("Exile").is_some() {
            ("Exile", "exiled")
        } else {
            ("Move", "moved")
        };
        remember::bind_moved(ctx, &mut put, verb, stem);
    }
    let mut names = Vec::new();
    if revealing {
        names.push(bind_revealed(ctx, &mut put));
    }
    effects.push(put);
    // Taking all of the cards that match "Card" leaves no rest; a move of the old ids would only chase cards already gone.
    if !(matches!(take, Take::All) && matches!(filter, None | Some("Card"))) {
        let mut rest = moves(&rest_ref, rest_to);
        if revealing {
            names.push(bind_revealed(ctx, &mut rest));
        }
        effects.push(rest);
    }
    if revealing {
        ctx.remembered = Some(match names.as_slice() {
            [one] => json!({ "Bound": one }),
            _ => {
                json!({"Each": [{"AnyOf": names.iter().map(|n| json!({"Is": {"Bound": n}})).collect::<Vec<_>>()}]})
            }
        });
    }
    Ok(effects)
}

/// Names the cards a move of a `Dig` moved, as they become, and returns the name.
fn bind_revealed(ctx: &mut Ctx, step: &mut Value) -> String {
    let verb = if step.get("Exile").is_some() {
        "Exile"
    } else {
        "Move"
    };
    remember::bind_moved(ctx, step, verb, "revealed");
    ctx.counted
        .remove(step[verb]["as"].as_str().unwrap_or_default());
    step[verb]["as"].as_str().unwrap_or_default().to_owned()
}

/// Every card looked at is exiled: the impulse shape, face down for no player to look at when `hide` seals them.
fn exile_top(ctx: &mut Ctx, p: &Params, who: &Value, count: &Value, hide: Hide) -> Value {
    let exiled = ctx.fresh("exiled");
    if p.get("RememberChanged") == Some("True") {
        ctx.remembered = Some(json!({ "Bound": exiled }));
    }
    let mut exile = json!({"ExileTop": {"who": who, "count": count, "as": exiled}});
    if hide == Hide::Sealed {
        exile["ExileTop"]["sealed"] = json!(true);
    }
    exile
}

/// Where a `Dig` puts the cards it takes and the rest.
pub(super) fn dig_places(p: &Params) -> Tr<(Goes, Goes)> {
    let to = match p.get("DestinationZone") {
        None | Some("Hand") => Goes::To(json!("Hand")),
        Some("Graveyard") => Goes::To(json!("Graveyard")),
        Some("Exile") => Goes::Exile,
        Some("Battlefield") => {
            Goes::To(json!({"Battlefield": {"tapped": p.get("Tapped") == Some("True")}}))
        }
        Some("Library") => {
            // Without a position the cards go to the bottom, as in Forge.
            let place = match p.get("LibraryPosition") {
                Some("0") => "Top",
                None | Some("-1") => "Bottom",
                _ => return unmapped("key:Dig.LibraryPosition"),
            };
            Goes::To(json!({"Library": {"place": place}}))
        }
        Some(other) => return unmapped(format!("key:Dig.DestinationZone={other}")),
    };
    // `LibraryPosition$` names where the cards taken go, so it says nothing for another destination: the rest go where
    // `DestinationZone2$` and `LibraryPosition2$` say, the bottom without them.
    let _ = p.get("LibraryPosition");
    // The order of the rest is asked for, random, or (`SkipReorder$`) left as it lies.
    let order = match (p.get("RestRandomOrder"), p.get("SkipReorder")) {
        (Some("True"), Some("True")) => {
            return unmapped("key:Dig.SkipReorder-with-RestRandomOrder");
        }
        (Some("True"), _) => json!("Random"),
        (_, Some("True")) => Value::Null,
        _ => json!("Chosen"),
    };
    let on_library = |place: &str| {
        let mut library = json!({"place": place});
        if !order.is_null() {
            library["order"] = order.clone();
        }
        Goes::To(json!({ "Library": library }))
    };
    let rest_to = match (p.get("DestinationZone2"), p.get("LibraryPosition2")) {
        (None | Some("Library"), None | Some("-1")) => on_library("Bottom"),
        (None | Some("Library"), Some("0")) => on_library("Top"),
        (Some("Graveyard"), None) => Goes::To(json!("Graveyard")),
        (Some("Hand"), None) => Goes::To(json!("Hand")),
        (Some("Exile"), None) => Goes::Exile,
        (Some(other), _) if !matches!(other, "Library" | "Graveyard" | "Hand" | "Exile") => {
            return unmapped(format!("key:Dig.DestinationZone2={other}"));
        }
        _ => return unmapped("key:Dig.LibraryPosition2"),
    };
    Ok((to, rest_to))
}

/// How many of the cards a `Dig` looks at are taken.
enum Take {
    Count(u64),
    /// A number the card counts (`ChangeNum$ X`).
    Counted(Value),
    Any,
    All,
}

impl Take {
    /// What `ChangeNum$` says.
    fn of(ctx: &Ctx, p: &Params) -> Tr<Take> {
        match p.get("ChangeNum") {
            None | Some("1") => Ok(Take::Count(1)),
            Some("Any") => Ok(Take::Any),
            Some("All") => Ok(Take::All),
            Some(n) => match n.parse::<u64>() {
                Ok(n) => Ok(Take::Count(n)),
                Err(_) => Ok(Take::Counted(amount(ctx, n)?)),
            },
        }
    }
}

/// `DigUntil`: reveal cards until some match, the match goes to one place and the revealed cards to another.
pub fn dig_until(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "DigUntil",
        &[
            "Valid",
            "Amount",
            "FoundDestination",
            "RevealedDestination",
            "RevealedLibraryPosition",
            "RevealRandomOrder",
            "Shuffle",
            "Defined",
            "ValidTgts",
            "Tapped",
            "GainControl",
            "RememberFound",
            "RememberRevealed",
            "ValidDescription",
        ],
    )?;
    // Forge remembers every card revealed, the one found included; where the found card goes elsewhere, "the cards
    // revealed this way" may mean the rest only (Spoils of the Vault), so only one destination for all is read.
    let remembers = p.get("RememberRevealed") == Some("True");
    if remembers && p.get("RememberFound") == Some("True") {
        return unmapped("key:DigUntil.RememberRevealed-with-RememberFound");
    }
    if remembers && p.has("FoundDestination") {
        return unmapped("key:DigUntil.RememberRevealed-with-a-found-destination");
    }
    let who = subject(ctx, p, "You", None)?;
    let found = ctx.fresh("found");
    let revealed = ctx.fresh("revealed");
    let mut find = json!({
        "who": who,
        "filter": selector_any(ctx, p.get("Valid").unwrap_or("Card"))?,
        "as": found,
        "revealed_as": revealed,
    });
    if let Some(n) = p.get("Amount").filter(|n| *n != "1") {
        find["count"] = amount(ctx, n)?;
    }
    let tapped = p.get("Tapped") == Some("True");
    // Without a destination the card found stays among the revealed ones, which move together.
    let found_to = match p.get("FoundDestination") {
        None => None,
        Some("Exile") => Some(Goes::Exile),
        Some("Hand") => Some(Goes::To(json!("Hand"))),
        Some("Graveyard") => Some(Goes::To(json!("Graveyard"))),
        Some("Battlefield") if p.get("GainControl") == Some("True") => Some(Goes::To(
            json!({"Battlefield": {"tapped": tapped, "under": "You"}}),
        )),
        Some("Battlefield") => Some(Goes::To(json!({"Battlefield": {"tapped": tapped}}))),
        Some(other) => return unmapped(format!("key:DigUntil.FoundDestination={other}")),
    };
    if p.get("RememberFound") == Some("True") {
        ctx.remembered = Some(json!({ "Bound": found }));
    }
    let order = if p.get("RevealRandomOrder") == Some("True") {
        "Random"
    } else {
        "Chosen"
    };
    let rest_to = match (
        p.get("RevealedDestination"),
        p.get("RevealedLibraryPosition"),
    ) {
        (Some("Library"), Some("-1")) => {
            Goes::To(json!({"Library": {"place": "Bottom", "order": order}}))
        }
        (Some("Library"), Some("0")) => Goes::To(json!({"Library": {"place": "Top"}})),
        (Some("Graveyard"), _) => Goes::To(json!("Graveyard")),
        (Some("Exile"), _) => Goes::Exile,
        (other, _) => return unmapped(format!("key:DigUntil.RevealedDestination={other:?}")),
    };
    let mut effects = vec![json!({ "RevealUntil": find })];
    if let Some(found_to) = found_to {
        effects.push(moves(&json!({"Bound": found}), found_to));
    }
    let mut rest = moves(&json!({"Bound": revealed}), rest_to);
    if remembers {
        // The cards as they are after the move, which the lines after it count.
        let verb = if rest.get("Exile").is_some() {
            "Exile"
        } else {
            "Move"
        };
        super::remember::bind_moved(ctx, &mut rest, verb, "revealed");
    }
    effects.push(rest);
    if p.get("Shuffle") == Some("True") {
        effects.push(json!({"Shuffle": {}}));
    }
    Ok(effects)
}

fn change_zone_all(ctx: &mut Ctx, p: &Params, origin: &str, destination: &str) -> Tr<Vec<Value>> {
    only(
        p,
        "ChangeZoneAll",
        &[
            "Origin",
            "GainControl",
            "Destination",
            "ChangeType",
            "Tapped",
            "LibraryPosition",
            "Shuffle",
            "RememberChanged",
            "ValidTgts",
            "Defined",
            "UseAllOriginZones",
            "RandomOrder",
            "Duration",
        ],
    )?;
    // Every zone of the origin is emptied, which is what a move of all of them does anyway.
    match p.get("UseAllOriginZones") {
        None | Some("True") => {}
        Some(_) => return unmapped("key:ChangeZoneAll.UseAllOriginZones"),
    }
    let zone = (origin != "Battlefield").then_some(origin);
    // The cards are read from the object the line targets (`TargetedCard.Self`): the target is declared first.
    let of_target = p.has("ValidTgts")
        && matches!(origin, "Battlefield")
        && p.get("ChangeType").is_some_and(|t| t.contains("Targeted"));
    if of_target {
        subject(ctx, p, "", None)?;
    }
    let mut filter = selector_in(ctx, p.get("ChangeType").unwrap_or("Card"), zone)?;
    if p.has("ValidTgts") && !of_target {
        // "Exile target player's graveyard": the cards of the zone that player owns.
        if !matches!(origin, "Graveyard" | "Hand" | "Exile" | "Library")
            || !matches!(p.get("ValidTgts"), Some("Player" | "Opponent"))
        {
            return unmapped("key:ChangeZoneAll.ValidTgts");
        }
        let owner = subject(ctx, p, "", Some(origin))?;
        filter.push(json!({"OwnedBy": owner}));
    }
    if p.has("Defined") {
        if p.has("ValidTgts") || !matches!(origin, "Graveyard" | "Hand" | "Exile" | "Library") {
            return unmapped("key:ChangeZoneAll.Defined");
        }
        filter.push(json!({"OwnedBy": zone_owner(ctx, p.get("Defined").unwrap_or_default())?}));
    }
    let mut effect = moves(&json!({ "Each": filter }), goes(ctx, p, destination)?);
    match p.get("Duration") {
        None => {}
        // "Exile all artifacts your opponents control until ~ leaves the battlefield": each returns when the source goes.
        Some("UntilHostLeavesPlay") if origin == "Battlefield" && effect.get("Exile").is_some() => {
            effect["Exile"]["duration"] = json!("WhileSourceOnBattlefield");
        }
        Some(_) => return unmapped("key:ChangeZoneAll.Duration"),
    }
    if remember::wanted(p, &["RememberChanged"]) {
        let (verb, stem) = if effect.get("Exile").is_some() {
            ("Exile", "exiled")
        } else {
            ("Move", "moved")
        };
        remember::bind_moved(ctx, &mut effect, verb, stem);
    }
    Ok(vec![effect])
}

/// `RearrangeTopOfLibrary`: the controller looks at the top cards of a library and puts them back on top in the order
/// they choose. With `MayShuffle$` the controller may then have that library shuffled ("You may shuffle", "You may have
/// that player shuffle"). A second player who orders stays a gap.
pub fn rearrange_top(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "RearrangeTopOfLibrary",
        &[
            "Defined",
            "NumCards",
            "ValidTgts",
            "TargetMin",
            "TargetMax",
            "MayShuffle",
        ],
    )?;
    let who = subject(ctx, p, "You", None)?;
    let count = amount(ctx, p.get("NumCards").unwrap_or("1"))?;
    let top = ctx.fresh("top");
    let mut effects = vec![
        json!({"LookAtTop": {"who": who, "count": count, "as": top}}),
        json!({"Move": {"what": {"Bound": top},
            "to": {"Library": {"place": "Top", "order": "Chosen"}}}}),
    ];
    match p.get("MayShuffle") {
        None => {}
        Some("True") => effects.push(json!({"May": {"effects": [{"Shuffle": {"who": who}}]}})),
        Some(_) => return unmapped("key:RearrangeTopOfLibrary.MayShuffle"),
    }
    Ok(effects)
}

/// How a `Dig` lays the cards it exiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hide {
    /// Face up.
    No,
    /// Face down, and their exiler may look at them (`WithMayLook$ True`).
    Held,
    /// Face down, and no player may look at them (CR 406.3).
    Sealed,
}

/// `ExileFaceDown$ True` on a `Dig`: the cards taken lie face down in exile; their exiler may look at them with
/// `WithMayLook$ True`, and no player may without it.
fn face_down(p: &Params) -> Tr<Hide> {
    // Read only for a face-down line: a face-up one reads its other keys where it uses them.
    let exiles = || {
        p.get("DestinationZone") == Some("Exile")
            && p.get("DestinationZone2") != Some("Exile")
            && p.get("Reveal") != Some("True")
    };
    match (p.get("ExileFaceDown"), p.get("WithMayLook")) {
        (None, None) => Ok(Hide::No),
        (Some("True"), Some("True")) if exiles() => Ok(Hide::Held),
        (Some("True"), None) if exiles() => Ok(Hide::Sealed),
        (Some("True"), Some("True")) => unmapped("key:Dig.ExileFaceDown-destination"),
        (Some("True"), _) => unmapped("key:Dig.ExileFaceDown"),
        _ => unmapped("key:Dig.WithMayLook"),
    }
}
