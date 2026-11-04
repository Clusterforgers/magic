//! Zone changes: `ChangeZone` of a target or of the card itself, the library search, and `ChangeZoneAll`.

use serde_json::{Value, json};

use super::amount::{amount, counter_kind};
use super::ctx::{Ctx, Tr, check_keys, only, unmapped};
use super::keyword::keyword_list;
use super::remember;
use super::valid::{defined, selector_any, selector_in, subject};
use crate::forge::script::Params;

/// The effects of `ChangeZone` and `ChangeZoneAll`.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Tr<Vec<Value>> {
    let origin = zone_word(p.get("Origin"), "Origin", true)?;
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
        ("ChangeZone", _) => change_zone(ctx, p, origin, destination),
        _ => change_zone_all(ctx, p, origin, destination),
    }
}

/// Whether the word is one of the hand, graveyard and exile, or a list of them (`Graveyard,Exile`).
fn hidden_origin(word: &str) -> bool {
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
        Some(list) if many && zone_list(list).is_some() => Ok(list),
        Some(other) => unmapped(format!("zone:{key}={other}")),
        None => unmapped(format!("key:ChangeZone.no-{key}")),
    }
}

/// Where the moved objects go, as a destination or the exile verb.
enum Goes {
    Exile,
    To(Value),
}

/// Where the moved objects go; a library placement follows `LibraryPosition$` (the top without one, as Forge) and a
/// `Shuffle$ True` puts them in shuffled.
fn goes(ctx: &Ctx, p: &Params, destination: &str) -> Tr<Goes> {
    goes_in(ctx, p, destination, false)
}

/// Where what a search found goes: a library placement there is the search's own shuffle and position, never a default.
fn goes_found(ctx: &Ctx, p: &Params, destination: &str) -> Tr<Goes> {
    goes_in(ctx, p, destination, true)
}

fn goes_in(ctx: &Ctx, p: &Params, destination: &str, searched: bool) -> Tr<Goes> {
    let tapped = p.get("Tapped") == Some("True");
    if destination != "Battlefield" {
        for key in [
            "GainControl",
            "WithCountersType",
            "Transformed",
            "AttachedTo",
        ] {
            if p.has(key) {
                return unmapped(format!("key:ChangeZone.{key}"));
            }
        }
    }
    Ok(match destination {
        "Exile" => Goes::Exile,
        "Hand" => Goes::To(json!("Hand")),
        "Graveyard" => Goes::To(json!("Graveyard")),
        "Battlefield" if p.has("AttachedTo") => {
            // The attached placement carries only the host: any other way of entering is a gap, not a drop.
            for key in ["Tapped", "GainControl", "WithCountersType", "Transformed"] {
                if p.has(key) {
                    return unmapped(format!("key:ChangeZone.AttachedTo-with-{key}"));
                }
            }
            let host = defined(ctx, p.get("AttachedTo").unwrap_or_default())?;
            Goes::To(json!({"BattlefieldAttached": {"to": host}}))
        }
        "Battlefield" => {
            let mut enters = json!({"tapped": tapped});
            match p.get("Transformed") {
                None | Some("False") => {}
                Some("True") => enters["transformed"] = json!(true),
                Some(_) => return unmapped("key:ChangeZone.Transformed"),
            }
            if p.get("GainControl") == Some("True") {
                enters["under"] = json!("You");
            }
            if p.has("WithCountersType") {
                enters["counters"] = with_counters(p)?;
            }
            Goes::To(json!({ "Battlefield": enters }))
        }
        "Library" => {
            let place = match (p.get("LibraryPosition"), p.get("Shuffle"), searched) {
                (_, Some("True"), false) | (None, Some("True"), _) => json!("Shuffled"),
                (Some("0"), ..) | (None, Some("False") | None, false) => json!("Top"),
                (Some("-1"), ..) => json!("Bottom"),
                (Some(n), ..) => match n.parse::<u32>() {
                    Ok(below) if below >= 1 => json!({ "Below": below }),
                    _ => return unmapped("key:ChangeZone.LibraryPosition"),
                },
                _ => return unmapped("key:ChangeZone.LibraryPosition"),
            };
            Goes::To(json!({"Library": {"place": place}}))
        }
        other => return unmapped(format!("zone:Destination={other}")),
    })
}

/// The counters a card enters with (`WithCountersType$ P1P1 | WithCountersAmount$ 2`): one kind with a fixed number, one
/// counter without an amount; a keyword counter, a list or a number that is not fixed stays a gap.
fn with_counters(p: &Params) -> Tr<Value> {
    let Some(kind) = p.get("WithCountersType").and_then(counter_kind) else {
        return unmapped("key:ChangeZone.WithCountersType");
    };
    let number = match p.get("WithCountersAmount") {
        None => Some(1),
        Some(text) => text.parse::<u32>().ok().filter(|n| *n >= 1),
    };
    match number {
        Some(n) => Ok(json!([[kind, n]])),
        None => unmapped("key:ChangeZone.WithCountersAmount"),
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

/// The modifications of the static the `StaticEffect$` SVar names ("It's a 1/1 Spirit creature with flying in addition to
/// its other types"): types added, a fixed power and toughness, a color and keywords. A word that takes something away or
/// adds a color stays a gap.
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
    for word in part.get("AddType").unwrap_or_default().split('&') {
        let word = word.trim();
        if word.is_empty() {
            continue;
        }
        mods.push(match word {
            "Legendary" | "Snow" | "Basic" => json!({ "AddSupertype": word }),
            _ if STATIC_CARD_TYPES.contains(&word) => json!({ "AddType": word }),
            _ => json!({ "AddSubtype": word }),
        });
    }
    if let Some(color) = part.get("SetColor") {
        if !matches!(color, "White" | "Blue" | "Black" | "Red" | "Green") {
            return unmapped("key:ChangeZone.StaticEffect=color");
        }
        mods.push(json!({ "SetColors": [color] }));
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
    let Some(step) = effects.iter_mut().find(|step| step.get("Move").is_some()) else {
        return unmapped("key:ChangeZone.StaticEffect=move");
    };
    let moved = match step["Move"].get("as").and_then(Value::as_str) {
        Some(name) => name.to_owned(),
        None => {
            let name = ctx.fresh("moved");
            step["Move"]["as"] = json!(name);
            name
        }
    };
    effects.push(json!({"Continuous": {
        "affected": {"Bound": moved},
        "mods": mods,
        "duration": "Forever",
    }}));
    Ok(())
}

fn moves(what: &Value, goes: Goes) -> Value {
    match goes {
        Goes::Exile => json!({"Exile": {"what": what}}),
        Goes::To(to) => json!({"Move": {"what": what, "to": to}}),
    }
}

fn change_zone(ctx: &mut Ctx, p: &Params, origin: &str, destination: &str) -> Tr<Vec<Value>> {
    only(
        p,
        "ChangeZone",
        &[
            "Origin",
            "Destination",
            "ValidTgts",
            "Defined",
            "TargetMin",
            "TargetMax",
            "LibraryPosition",
            "Shuffle",
            "Tapped",
            "RememberChanged",
            "RememberLKI",
            "GainControl",
            "WithCountersType",
            "WithCountersAmount",
            "RememberTargets",
            "Transformed",
            "AttachedTo",
            "StaticEffect",
        ],
    )?;
    let what = subject(ctx, p, "Self", Some(origin))?;
    let mut effect = moves(&what, goes(ctx, p, destination)?);
    if remember::wanted(p, &["RememberChanged", "RememberLKI", "RememberTargets"]) {
        let (verb, stem) = if effect.get("Exile").is_some() {
            ("Exile", "exiled")
        } else {
            ("Move", "moved")
        };
        remember::bind_moved(ctx, &mut effect, verb, stem);
    }
    let mut effects = vec![effect];
    with_static_effect(ctx, p, destination, &mut effects)?;
    Ok(effects)
}

/// A card of the player's own hand, graveyard or exile that the player picks and moves ("put a creature card from your hand
/// onto the battlefield"): a choice among the matching cards, then the move.
fn choose_and_move(ctx: &mut Ctx, p: &Params, origin: &str, destination: &str) -> Tr<Vec<Value>> {
    only(
        p,
        "ChangeZone.Choose",
        &[
            "Origin",
            "Destination",
            "ChangeType",
            "ChangeNum",
            "Hidden",
            "Mandatory",
            "Optional",
            "AtRandom",
            "Tapped",
            "LibraryPosition",
            "Shuffle",
            "DefinedPlayer",
            "Chooser",
            "ValidTgts",
            "RememberChanged",
            "GainControl",
            "WithCountersType",
            "WithCountersAmount",
            "Transformed",
            "AttachedTo",
            "StaticEffect",
        ],
    )?;
    let who = defined_player(ctx, p)?;
    let (owner, chooser) = (who.owner.clone(), who.chooser.clone());
    let mut filter = selector_in(ctx, p.get("ChangeType").unwrap_or("Card"), Some(origin))?;
    if origin == "Battlefield" {
        // On the battlefield the line's own words say whose permanents: an unstated owner is not assumed.
        let says_whose = filter
            .iter()
            .any(|pred| pred.get("ControlledBy").is_some() || pred.get("OwnedBy").is_some());
        if !says_whose || owner != json!("You") {
            return unmapped("key:ChangeZone.Choose.Battlefield-owner");
        }
    } else if filter.iter().any(|pred| pred.get("OwnedBy").is_some()) {
        if owner != json!("You") {
            return unmapped("key:ChangeZone.Choose.DefinedPlayer");
        }
    } else {
        filter.push(json!({"OwnedBy": owner}));
    }
    let max = amount(ctx, p.get("ChangeNum").unwrap_or("1"))?;
    let min = if p.get("Mandatory") == Some("True") && p.get("Optional") != Some("True") {
        max.clone()
    } else {
        json!(0)
    };
    let bind = ctx.fresh("picked");
    let chosen = json!({ "Bound": bind });
    let choose = if p.get("AtRandom") == Some("True") {
        if p.get("Optional") == Some("True") {
            return unmapped("key:ChangeZone.Choose.AtRandom-optional");
        }
        json!({"ChooseAtRandom": {"from": {"Each": filter}, "count": max, "as": bind}})
    } else {
        json!({"Choose": {"who": chooser, "from": {"Each": filter}, "count": {"min": min, "max": max}, "as": bind}})
    };
    let mut moved = moves(&chosen, goes(ctx, p, destination)?);
    if remember::wanted(p, &["RememberChanged"]) {
        let (verb, stem) = if moved.get("Exile").is_some() {
            ("Exile", "exiled")
        } else {
            ("Move", "moved")
        };
        remember::bind_moved(ctx, &mut moved, verb, stem);
    }
    match who.each {
        Some((over, name)) => {
            if remember::wanted(p, &["RememberChanged"]) || p.has("StaticEffect") {
                return unmapped("key:ChangeZone.Choose.RememberChanged-for-each-player");
            }
            Ok(vec![
                json!({"RepeatEach": {"over": over, "as": name, "effects": [choose, moved]}}),
            ])
        }
        None => {
            let mut steps = vec![choose, moved];
            with_static_effect(ctx, p, destination, &mut steps)?;
            Ok(steps)
        }
    }
}

/// Whose cards a choice from a hidden zone is among and who chooses.
struct Who {
    owner: Value,
    chooser: Value,
    /// The players the choice is made for in turn, and the name the player of each pass is bound to.
    each: Option<(Value, String)>,
}

/// The player words a `DefinedPlayer$` or `Chooser$` may be when the line has no target.
const PLAYER_WORDS: [&str; 6] = [
    "TriggeredPlayer",
    "TriggeredTarget",
    "TriggeredCardController",
    "RememberedController",
    "ChosenPlayer",
    "Remembered",
];

/// `DefinedPlayer$` and `Chooser$` of a choice from a hidden zone: whose cards are chosen from (you, the one player the
/// line targets with `ValidTgts$ Player` or `Opponent`, a player a word names, or each of them in turn) and who chooses
/// (the same player unless `Chooser$` says otherwise).
fn defined_player(ctx: &mut Ctx, p: &Params) -> Tr<Who> {
    let targets = p.has("ValidTgts");
    let one_target = matches!(p.get("ValidTgts"), Some("Player" | "Opponent"))
        && !p.has("TargetMin")
        && !p.has("TargetMax");
    let mut each = None;
    let (owner, targeted) = match (p.get("DefinedPlayer"), targets) {
        (None | Some("You"), false) => (json!("You"), false),
        (Some("Targeted" | "TargetedPlayer"), true) if one_target => {
            (subject(ctx, p, "You", None)?, true)
        }
        (Some(word @ ("Player" | "Player.Opponent")), false) => {
            let name = ctx.fresh("player");
            let owner = json!({ "Bound": name });
            each = Some((defined(ctx, word)?, name));
            (owner, false)
        }
        (Some(word), false) if PLAYER_WORDS.contains(&word) => (defined(ctx, word)?, false),
        (None, true) => return unmapped("key:ChangeZone.Choose.ValidTgts"),
        _ => return unmapped("key:ChangeZone.Choose.DefinedPlayer"),
    };
    let chooser = match p.get("Chooser") {
        None => owner.clone(),
        Some("You") => json!("You"),
        Some("Targeted") if targeted => owner.clone(),
        Some(word) if PLAYER_WORDS.contains(&word) && !targets && each.is_none() => {
            defined(ctx, word)?
        }
        Some(_) => return unmapped("key:ChangeZone.Choose.Chooser"),
    };
    Ok(Who {
        owner,
        chooser,
        each,
    })
}

/// The library search: the search, the move of what was found, then the shuffle.
fn search(ctx: &mut Ctx, p: &Params, destination: &str) -> Tr<Vec<Value>> {
    only(
        p,
        "ChangeZone.Search",
        &[
            "Origin",
            "Destination",
            "ChangeType",
            "ChangeNum",
            "Hidden",
            "Mandatory",
            "Tapped",
            "ShuffleNonMandatory",
            "RememberChanged",
            "Reveal",
            "LibraryPosition",
            "Shuffle",
            "Optional",
            "OriginAlternative",
            "GainControl",
            "WithCountersType",
            "WithCountersAmount",
        ],
    )?;
    let filter = selector_any(ctx, p.get("ChangeType").unwrap_or("Card"))?;
    let filter_is_open = filter.is_empty();
    let bind = ctx.fresh("found");
    let mut find = json!({"filter": filter, "as": bind});
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
    if !listed.is_empty() && p.has("OriginAlternative") {
        return unmapped("key:ChangeZone.Search.OriginAlternative");
    }
    let alternative = p
        .get("OriginAlternative")
        .or((!listed.is_empty()).then_some(listed.as_str()));
    if let Some(list) = alternative {
        let also: Vec<&str> = list.split(',').collect();
        if !also
            .iter()
            .all(|zone| matches!(*zone, "Graveyard" | "Hand"))
            || (1..also.len()).any(|at| also[..at].contains(&also[at]))
        {
            return unmapped("key:ChangeZone.Search.OriginAlternative");
        }
        find["also"] = json!(also);
    }
    match p.get("ChangeNum") {
        None | Some("1") => {}
        Some(n) => find["count"] = json!({"min": 0, "max": amount(ctx, n)?}),
    }
    if p.get("Reveal") == Some("True") || (destination == "Hand" && !filter_is_open) {
        find["reveal"] = json!(true);
    }
    let found = json!({ "Bound": bind });
    if p.get("RememberChanged") == Some("True") {
        ctx.remembered = Some(found.clone());
    }
    let search = json!({ "Search": find });
    let put = moves(&found, goes_found(ctx, p, destination)?);
    // Searching only another zone leaves the library as it was: "if you search your library this way, shuffle".
    let shuffle = if alternative.is_some() {
        json!({"If": {"condition": {"Searched": {"zone": "Library"}}, "then": [{"Shuffle": {}}]}})
    } else {
        json!({"Shuffle": {}})
    };
    let mut effects = match (destination, p.get("Shuffle")) {
        // The shuffle comes first when the card goes back on top: "then shuffle and put that card on top".
        ("Library", _) => vec![search, shuffle, put],
        (_, Some("False")) => vec![search, put],
        _ => vec![search, put, shuffle],
    };
    if p.get("Optional") == Some("True") {
        // A "may" the printed description does not carry is Forge's narrowing, not the card's.
        if let Some(text) = p.get("SpellDescription")
            && !text
                .to_lowercase()
                .split(|c: char| !c.is_alphabetic())
                .any(|w| w == "may")
        {
            return unmapped("key:ChangeZone.Optional");
        }
        effects = vec![json!({"May": {"effects": effects}})];
    }
    Ok(effects)
}

/// The keys of a `Dig` line that are read.
const DIG_KEYS: [&str; 19] = [
    "DigNum",
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
    "RememberChanged",
];

/// `Dig`: look at the top cards, take some, put the rest somewhere ("look at the top four cards of your library; you may
/// reveal a creature card from among them and put it into your hand; put the rest on the bottom in any order").
pub fn dig(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "Dig", &DIG_KEYS)?;
    if p.get("NoReveal") == Some("True") && p.get("Reveal") == Some("True") {
        return unmapped("key:Dig.NoReveal-with-Reveal");
    }
    let who = subject(ctx, p, "You", None)?;
    let count = amount(ctx, p.get("DigNum").unwrap_or("1"))?;
    if p.get("ChangeNum") == Some("All")
        && p.get("DestinationZone") == Some("Exile")
        && !p.has("ChangeValid")
    {
        return Ok(vec![exile_top(ctx, p, &who, &count)]);
    }
    let top = ctx.fresh("top");
    let kept = ctx.fresh("kept");
    let rest = ctx.fresh("rest");
    let from_top = json!({ "Bound": top });
    let take = Take::of(p)?;
    let filter = p.get("ChangeValid");
    let may = p.get("Optional") == Some("True") || filter.is_some();
    // "Put all of them" has no choice: what matches moves, and a "may" would be all or nothing.
    if matches!(take, Take::All) && p.get("Optional") == Some("True") {
        return unmapped("key:Dig.ChangeNum");
    }
    let matching = match filter {
        Some(valid) => {
            let mut selector = vec![json!({"In": "Library"}), json!({"Is": from_top})];
            selector.extend(selector_any(ctx, valid)?);
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
        Take::Count(_) | Take::All => {}
    }
    let (to, rest_to) = dig_places(p)?;
    let rest_ref = if filter.is_some() {
        from_top.clone()
    } else {
        json!({ "Bound": rest })
    };
    if p.get("RememberChanged") == Some("True") && !matches!(take, Take::All) {
        ctx.remembered = Some(json!({ "Bound": kept }));
    }
    let mut effects = vec![json!({"LookAtTop": {"who": who, "count": count, "as": top}})];
    let moved = if matches!(take, Take::All) {
        matching
    } else {
        effects.push(json!({ "Choose": choose }));
        json!({ "Bound": kept })
    };
    if p.get("Reveal") == Some("True") {
        let shown = if matches!(take, Take::All) {
            from_top
        } else {
            moved.clone()
        };
        effects.push(json!({"Reveal": {"what": shown}}));
    }
    let mut put = moves(&moved, to);
    if matches!(take, Take::All) && p.get("RememberChanged") == Some("True") {
        let (verb, stem) = if put.get("Exile").is_some() {
            ("Exile", "exiled")
        } else {
            ("Move", "moved")
        };
        remember::bind_moved(ctx, &mut put, verb, stem);
    }
    effects.push(put);
    if !(matches!(take, Take::All) && filter.is_none()) {
        effects.push(moves(&rest_ref, rest_to));
    }
    Ok(effects)
}

/// Every card looked at is exiled: the impulse shape.
fn exile_top(ctx: &mut Ctx, p: &Params, who: &Value, count: &Value) -> Value {
    let exiled = ctx.fresh("exiled");
    if p.get("RememberChanged") == Some("True") {
        ctx.remembered = Some(json!({ "Bound": exiled }));
    }
    json!({"ExileTop": {"who": who, "count": count, "as": exiled}})
}

/// Where a `Dig` puts the cards it takes and the rest.
fn dig_places(p: &Params) -> Tr<(Goes, Goes)> {
    let to = match p.get("DestinationZone") {
        None | Some("Hand") => Goes::To(json!("Hand")),
        Some("Graveyard") => Goes::To(json!("Graveyard")),
        Some("Exile") => Goes::Exile,
        Some("Battlefield") => {
            Goes::To(json!({"Battlefield": {"tapped": p.get("Tapped") == Some("True")}}))
        }
        Some("Library") => {
            let place = match p.get("LibraryPosition") {
                Some("0") => "Top",
                Some("-1") => "Bottom",
                _ => return unmapped("key:Dig.LibraryPosition"),
            };
            Goes::To(json!({"Library": {"place": place}}))
        }
        Some(other) => return unmapped(format!("key:Dig.DestinationZone={other}")),
    };
    // The position names where the cards taken go, so it is read only for a library destination.
    if p.has("LibraryPosition") && p.get("DestinationZone") != Some("Library") {
        return unmapped("key:Dig.LibraryPosition");
    }
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
    Any,
    All,
}

impl Take {
    /// What `ChangeNum$` says.
    fn of(p: &Params) -> Tr<Take> {
        match p.get("ChangeNum") {
            None | Some("1") => Ok(Take::Count(1)),
            Some("Any") => Ok(Take::Any),
            Some("All") => Ok(Take::All),
            Some(n) => n
                .parse::<u64>()
                .map_or_else(|_| unmapped("key:Dig.ChangeNum"), |n| Ok(Take::Count(n))),
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
            "ValidDescription",
        ],
    )?;
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
    let found_to = match p.get("FoundDestination") {
        Some("Exile") => Goes::Exile,
        Some("Hand") => Goes::To(json!("Hand")),
        Some("Graveyard") => Goes::To(json!("Graveyard")),
        Some("Battlefield") if p.get("GainControl") == Some("True") => {
            Goes::To(json!({"Battlefield": {"tapped": tapped, "under": "You"}}))
        }
        Some("Battlefield") => Goes::To(json!({"Battlefield": {"tapped": tapped}})),
        other => return unmapped(format!("key:DigUntil.FoundDestination={other:?}")),
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
    let mut effects = vec![
        json!({ "RevealUntil": find }),
        moves(&json!({"Bound": found}), found_to),
        moves(&json!({"Bound": revealed}), rest_to),
    ];
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
            "Destination",
            "ChangeType",
            "Tapped",
            "LibraryPosition",
            "Shuffle",
            "RememberChanged",
        ],
    )?;
    if origin == "Library" {
        return unmapped("key:ChangeZoneAll.Origin=Library");
    }
    let zone = (origin != "Battlefield").then_some(origin);
    let filter = selector_in(ctx, p.get("ChangeType").unwrap_or("Card"), zone)?;
    let mut effect = moves(&json!({ "Each": filter }), goes(ctx, p, destination)?);
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
