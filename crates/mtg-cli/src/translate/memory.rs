//! Memory: what an effect remembers and later reads back. Forge's named slots map to two memories by one rule: a
//! slot read on the line that wrote it is a bound name (`Bound`); a slot read on another line of the card is the
//! source's memory (`Effect::Remember`, `Ref::Remembered`).

use std::collections::BTreeSet;

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, only, unmapped};
use super::valid::defined;
use crate::forge::script::{Face, Params};

/// The two memories of a Forge card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// `Remember*` keys and `Remembered` words.
    Remembered,
    /// `Imprint*` keys and `Imprinted` words.
    Imprinted,
}

impl Slot {
    /// The name of the memory in the DSL.
    pub const fn name(self) -> &'static str {
        match self {
            Slot::Remembered => "remembered",
            Slot::Imprinted => "imprinted",
        }
    }

    /// The word that reads the slot.
    const fn word(self) -> &'static str {
        match self {
            Slot::Remembered => "Remembered",
            Slot::Imprinted => "Imprinted",
        }
    }

    /// The slot a writing key (`RememberChanged`, `ImprintFound`) belongs to.
    fn of_key(key: &str) -> Option<Slot> {
        if key.starts_with("Remember") && key != "RememberObjects" {
            Some(Slot::Remembered)
        } else if key.starts_with("Imprint") && key != "ImprintOnHost" {
            Some(Slot::Imprinted)
        } else {
            None
        }
    }
}

/// Which memories a card reads on a line other than the one that wrote them, and so keeps on the source.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Plan {
    /// `Remembered` is read across lines.
    pub remembered: bool,
    /// `Imprinted` is read across lines.
    pub imprinted: bool,
    /// What the source chose as it entered.
    pub entered: Entered,
}

/// The choices a permanent's controller made as it entered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Entered {
    /// A colour.
    pub color: bool,
    /// A creature type.
    pub subtype: bool,
    /// A card name.
    pub name: bool,
}

impl Plan {
    /// Whether the slot lives on the source.
    pub fn crosses(self, slot: Slot) -> bool {
        match slot {
            Slot::Remembered => self.remembered,
            Slot::Imprinted => self.imprinted,
        }
    }
}

/// The first pass over a face: for each slot, whether a line reads it that does not write it.
pub fn plan(face: &Face) -> Plan {
    let lines = face
        .keywords
        .iter()
        .chain(&face.abilities)
        .chain(&face.triggers)
        .chain(&face.statics)
        .chain(&face.replacements);
    let mut plan = Plan::default();
    for line in lines {
        let parts = group(face, line);
        for slot in [Slot::Remembered, Slot::Imprinted] {
            if parts.iter().any(|part| writes(part, slot)) {
                continue;
            }
            if parts.iter().any(|part| reads(part, slot)) {
                match slot {
                    Slot::Remembered => plan.remembered = true,
                    Slot::Imprinted => plan.imprinted = true,
                }
            }
        }
    }
    // A slot read across lines needs a writer somewhere; one nobody writes stays a gap where it is read.
    let written = |slot| {
        face.keywords
            .iter()
            .chain(&face.abilities)
            .chain(&face.triggers)
            .chain(&face.statics)
            .chain(&face.replacements)
            .flat_map(|line| group(face, line))
            .any(|part| writes(&part, slot))
    };
    plan.remembered &= written(Slot::Remembered);
    plan.imprinted &= written(Slot::Imprinted);
    entering(face, &mut plan);
    plan
}

/// The text of a line and of every named part it reaches.
fn group(face: &Face, line: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = vec![line.to_owned()];
    let mut next = vec![line.to_owned()];
    while let Some(text) = next.pop() {
        for value in values(&text) {
            let name = value.trim();
            if let Some(part) = face.svars.get(name)
                && seen.insert(name.to_owned())
            {
                out.push(part.clone());
                next.push(part.clone());
            }
        }
    }
    out
}

/// The values of the `Key$ Value` parts of a text.
fn values(text: &str) -> impl Iterator<Item = &str> {
    text.split('|')
        .filter_map(|part| part.split_once('$').map(|(_, value)| value))
}

/// Whether a part reads the slot in any value.
fn reads(text: &str, slot: Slot) -> bool {
    values(text).any(|value| value.contains(slot.word()))
}

/// Whether a part writes the slot: a `Remember*`/`Imprint*` key set to `True`, or the `Remember`/`Imprint` API.
fn writes(text: &str, slot: Slot) -> bool {
    text.split('|').any(|part| {
        let Some((key, value)) = part.split_once('$') else {
            return false;
        };
        let (key, value) = (key.trim(), value.trim());
        (value == "True" && Slot::of_key(key) == Some(slot))
            || (key == "ImprintCards" && slot == Slot::Imprinted)
            || (matches!(key, "SP" | "AB" | "DB")
                && value
                    == match slot {
                        Slot::Remembered => "Remember",
                        Slot::Imprinted => "Imprint",
                    })
    })
}

/// The objects a slot holds on the line being translated: the name an earlier step of the chain bound, else the memory of
/// the source when the card reads it across lines.
pub fn slot_ref(ctx: &Ctx, slot: Slot) -> Option<Value> {
    slot_ref_loose(ctx, slot).filter(|held| !is_scoped(ctx, held))
}

/// Whether `held` is a name bound inside a condition or a "may", whose objects a later step cannot use.
pub fn is_scoped(ctx: &Ctx, held: &Value) -> bool {
    held.get("Bound")
        .and_then(Value::as_str)
        .is_some_and(|name| ctx.scoped.contains(name))
}

/// [`slot_ref`] for a step that only counts the objects or tests that there are some: a name bound inside a condition or
/// a "may" reads as empty when that part did not run.
pub fn slot_ref_loose(ctx: &Ctx, slot: Slot) -> Option<Value> {
    let bound = match slot {
        Slot::Remembered => ctx.remembered.clone(),
        Slot::Imprinted => ctx.imprinted.clone(),
    };
    bound.or_else(|| {
        ctx.memory
            .crosses(slot)
            .then(|| json!({"Remembered": {"on": "This", "slot": slot.name()}}))
    })
}

/// `Defined$ Remembered` and kin.
pub fn remembered(ctx: &Ctx) -> Tr<Value> {
    slot_ref(ctx, Slot::Remembered)
        .map_or_else(|| unmapped("defined:Remembered-without-a-change"), Ok)
}

/// The characteristic a `Remembered$Card…` word reads.
pub fn stat_word(what: &str) -> Option<&'static str> {
    match what {
        "CardPower" => Some("Power"),
        "CardToughness" => Some("Toughness"),
        "CardManaCost" => Some("ManaValue"),
        _ => None,
    }
}

/// `Remembered$CardPower` and kin: the characteristic added up over everything the slot holds (a choice of any number of
/// creatures has a total), looking in every zone like [`remembered_count`]. One target stays that object's own value.
pub fn remembered_total(ctx: &Ctx, what: &str) -> Tr<Value> {
    let (Some(held), Some(stat)) = (slot_ref(ctx, Slot::Remembered), stat_word(what)) else {
        return unmapped("amount:Remembered-without-a-change");
    };
    let several = held.get("Bound").is_some() || held.get("Remembered").is_some();
    if !several
        || held
            .get("Bound")
            .and_then(Value::as_str)
            .is_some_and(|n| ctx.counted.contains(n))
    {
        return super::amount::targeted_amount("Remembered", what, &held);
    }
    let zones: Vec<Value> = [
        "Battlefield",
        "Hand",
        "Library",
        "Graveyard",
        "Exile",
        "Stack",
    ]
    .iter()
    .map(|zone| json!({"In": zone}))
    .collect();
    Ok(json!({"Total": {"of": [{"Is": held}, {"AnyOf": zones}], "stat": stat}}))
}

/// `Remembered$Amount`: how many objects the slot holds. They sit in the zone the step put them in (CR 400.7), which the
/// selector does not know, so it looks in every zone.
pub fn remembered_count(ctx: &Ctx) -> Tr<Value> {
    let Some(held) = slot_ref_loose(ctx, Slot::Remembered) else {
        return unmapped("amount:Remembered-without-a-change");
    };
    // A token a step moved is gone, so the engine keeps the number the step moved (CR 111.7, 400.7).
    if let Some(name) = held
        .get("Bound")
        .and_then(Value::as_str)
        .filter(|name| ctx.counted.contains(*name))
    {
        return Ok(json!({"Bound": format!("{name}~moved")}));
    }
    let zones: Vec<Value> = [
        "Battlefield",
        "Hand",
        "Library",
        "Graveyard",
        "Exile",
        "Stack",
    ]
    .iter()
    .map(|zone| json!({"In": zone}))
    .collect();
    Ok(json!({"Count": [{"Is": held}, {"AnyOf": zones}]}))
}

/// `Remembered$Valid <filter>`: how many of the objects the slot holds match the filter, as they are now.
pub fn remembered_matching(ctx: &Ctx, filter: &str) -> Tr<Value> {
    let Some(held) = slot_ref(ctx, Slot::Remembered) else {
        return unmapped("amount:Remembered-without-a-change");
    };
    let mut selector = vec![
        json!({"Is": held}),
        json!({"AnyOf": ZONES.map(|zone| json!({"In": zone}))}),
    ];
    selector.extend(super::valid::selector_any(ctx, filter)?);
    Ok(json!({"Count": selector}))
}

/// `RememberedController`: whoever controlled the objects as the step named them. What a step moved is another object
/// (CR 400.7) whose controller is its owner, so the reading is of the target the step named, which keeps the controller
/// it last had; a name the translator cannot trace back to a target stays a gap.
pub fn remembered_controller(ctx: &Ctx) -> Tr<Value> {
    if ctx
        .remembered
        .as_ref()
        .is_some_and(|held| is_scoped(ctx, held))
    {
        return unmapped("defined:RememberedController");
    }
    let before = match &ctx.remembered {
        Some(Value::Object(map)) if map.len() == 1 => {
            match map.get("Bound").and_then(Value::as_str) {
                Some(name) => ctx.moved_from.get(name).cloned(),
                None => map.get("Target").map(|_| Value::Object(map.clone())),
            }
        }
        _ => None,
    };
    match before {
        Some(what) => Ok(json!({"ControllerOf": what})),
        None => unmapped("defined:RememberedController"),
    }
}

/// `Defined$ Imprinted`.
pub fn imprinted(ctx: &Ctx) -> Tr<Value> {
    slot_ref(ctx, Slot::Imprinted)
        .map_or_else(|| unmapped("defined:Imprinted-without-a-change"), Ok)
}

/// `Card.IsRemembered` and `Card.IsImprinted`: the object is one of the slot's.
pub fn is_in(ctx: &Ctx, slot: Slot) -> Tr<Vec<Value>> {
    let held = match slot {
        Slot::Remembered => ctx.pinned.clone().or_else(|| slot_ref(ctx, slot)),
        Slot::Imprinted => slot_ref(ctx, slot),
    };
    match held {
        Some(who) => Ok(vec![json!({"Is": who})]),
        None => unmapped(format!("valid:Is{}", slot.word())),
    }
}

/// Every zone an object can be in: a remembered object is looked for where it is now.
const ZONES: [&str; 6] = [
    "Battlefield",
    "Hand",
    "Library",
    "Graveyard",
    "Exile",
    "Stack",
];

/// The key an `Imprint*` key reads as when a verb binds what it moved: the verb remembers, and the line imprints.
const IMPRINT_AS: [(&str, &str, &str); 11] = [
    ("Token", "ImprintTokens", "RememberTokens"),
    ("CopyPermanent", "ImprintTokens", "RememberTokens"),
    ("ChooseCard", "ImprintChosen", "RememberChosen"),
    ("ChangeZone", "Imprint", "RememberChanged"),
    ("ChangeZoneAll", "Imprint", "RememberChanged"),
    ("Dig", "Imprint", "RememberChanged"),
    ("DigUntil", "ImprintFound", "RememberFound"),
    ("DigUntil", "ImprintRevealed", "RememberRevealed"),
    ("PeekAndReveal", "ImprintRevealed", "RememberRevealed"),
    ("Reveal", "ImprintRevealed", "RememberRevealed"),
    ("Mill", "Imprint", "RememberMilled"),
];

/// The keys that remember the objects the line acts on (its target or `Defined$`), for verbs that cannot fail to act.
const ACTED_ON: [(&str, &str); 5] = [
    ("DealDamage", "RememberDamaged"),
    ("PutCounter", "RememberPut"),
    ("PutCounter", "RememberCards"),
    ("Tap", "RememberTapped"),
    ("GainControl", "RememberControlled"),
];

/// One line of an ability. `run` maps the verb. The `Remember*`/`Imprint*` keys bind what the verb acted on to the
/// slot; a card that reads the slot on another line also gets an `Effect::Remember` for it.
pub fn line(
    ctx: &mut Ctx,
    p: &Params,
    api: &str,
    run: fn(&mut Ctx, &Params, &str) -> Tr<Vec<Value>>,
) -> Tr<Vec<Value>> {
    match api {
        "Remember" => return remember_api(ctx, p, Slot::Remembered),
        "Imprint" => return remember_api(ctx, p, Slot::Imprinted),
        "Cleanup" => return cleanup(ctx, p, run),
        _ => {}
    }
    if let Some((rest, asked)) = super::forget::split(p) {
        let before = ctx.remembered.clone();
        let effects = line(ctx, &rest, api, run)?;
        return super::forget::apply(ctx, &asked, &before, effects);
    }
    if let Some(word) = p.get("ImprintCards") {
        return imprint_cards(ctx, p, api, word, run);
    }
    if let Some((rest, kind)) = super::number::split(p, api) {
        let effects = line(ctx, &rest, api, run)?;
        return super::number::apply(ctx, kind, effects);
    }
    let keys: Vec<(String, Slot)> = p
        .iter()
        .filter(|(_, value)| *value == "True")
        .filter_map(|(key, _)| Some((key.to_owned(), Slot::of_key(key)?)))
        .collect();
    if keys.is_empty() {
        return run(ctx, p, api);
    }
    let before = (ctx.remembered.clone(), ctx.imprinted.clone());
    let mut line = p.without(&[]);
    let mut acted: Vec<Slot> = Vec::new();
    let mut moved: Vec<Slot> = Vec::new();
    for (key, slot) in &keys {
        if let Some(&(_, _, own)) = IMPRINT_AS
            .iter()
            .find(|(verb, imprint, _)| *verb == api && imprint == key)
        {
            if keys.iter().any(|(other, _)| other == own) {
                return unmapped(format!("key:{api}.{key}-with-{own}"));
            }
            line = rename(&line, key, own);
            moved.push(*slot);
        } else if ACTED_ON.contains(&(api, key.as_str())) && !p.has("CardChoices") {
            // Damage divided or dealt to several targets names the ones that got some; one object is bound here.
            if p.has("DividedAsYouChoose") || p.has("TargetMax") {
                return unmapped(format!("key:{api}.{key}-with-several-objects"));
            }
            line = line.without(&[key]);
            acted.push(*slot);
        } else if key == "RememberTargets" && !matches!(api, "ChangeZone" | "Destroy") {
            if !p.has("ValidTgts") {
                return unmapped("key:RememberTargets-without-targets");
            }
            line = line.without(&[key]);
            acted.push(*slot);
        } else if *slot == Slot::Imprinted {
            return unmapped(format!("key:{api}.{key}"));
        }
    }
    let mut effects = run(ctx, &line, api)?;
    for slot in &acted {
        let what = if p.has("ValidTgts") {
            ctx.last_target()?
        } else if let Some(word) = p.get("Defined") {
            defined(ctx, word)?
        } else {
            return unmapped(format!("key:{api}.{}-without-an-object", slot.name()));
        };
        set(ctx, *slot, Some(what));
    }
    for slot in &moved {
        // The verb bound what it moved to `remembered`; the line keeps it as imprinted.
        if ctx.remembered == before.0 {
            return unmapped(format!("key:{api}.Imprint-nothing-bound"));
        }
        let held = std::mem::replace(&mut ctx.remembered, before.0.clone());
        set(ctx, *slot, held);
    }
    // What the line wrote is also the source's memory when another line reads it.
    let mut written: Vec<Slot> = keys.iter().map(|(_, slot)| *slot).collect();
    written.dedup();
    for slot in written {
        if ctx.memory.crosses(slot)
            && let Some(what) = held(ctx, slot)
            && Some(&what) != held_before(&before, slot).as_ref()
        {
            effects.push(remember_step(&what, slot));
        }
    }
    Ok(effects)
}

/// `ImprintCards$ <word>`: the line imprints the objects the word names. A word that stands for the line's own target is read
/// once the line has declared it; the others are read first, so the line's own parts (a trigger, a shield) can read the slot.
fn imprint_cards(
    ctx: &mut Ctx,
    p: &Params,
    api: &str,
    word: &str,
    run: fn(&mut Ctx, &Params, &str) -> Tr<Vec<Value>>,
) -> Tr<Vec<Value>> {
    let rest = p.without(&["ImprintCards"]);
    let own_target = matches!(word, "ThisTargetedCard" | "Targeted") && p.has("ValidTgts");
    if own_target {
        let mut effects = line(ctx, &rest, api, run)?;
        let what = ctx.last_target()?;
        set(ctx, Slot::Imprinted, Some(what.clone()));
        if ctx.memory.crosses(Slot::Imprinted) {
            effects.push(remember_step(&what, Slot::Imprinted));
        }
        return Ok(effects);
    }
    let what = match word {
        "Self" | "Remembered" | "Imprinted" | "Targeted" | "ParentTarget" | "Enchanted"
        | "ChosenCard" | "TriggeredCard" | "TriggeredSource" | "TriggeredAttacker" => {
            defined(ctx, word)?
        }
        other => return unmapped(format!("key:{api}.ImprintCards={other}")),
    };
    let before = ctx.imprinted.clone();
    set(ctx, Slot::Imprinted, Some(what.clone()));
    let mut effects = Vec::new();
    if ctx.memory.crosses(Slot::Imprinted) {
        effects.push(remember_step(&what, Slot::Imprinted));
    }
    match line(ctx, &rest, api, run) {
        Ok(more) => effects.extend(more),
        Err(gap) => {
            ctx.imprinted = before;
            return Err(gap);
        }
    }
    Ok(effects)
}

fn held(ctx: &Ctx, slot: Slot) -> Option<Value> {
    match slot {
        Slot::Remembered => ctx.remembered.clone(),
        Slot::Imprinted => ctx.imprinted.clone(),
    }
}

fn held_before(before: &(Option<Value>, Option<Value>), slot: Slot) -> Option<Value> {
    match slot {
        Slot::Remembered => before.0.clone(),
        Slot::Imprinted => before.1.clone(),
    }
}

fn set(ctx: &mut Ctx, slot: Slot, what: Option<Value>) {
    match slot {
        Slot::Remembered => ctx.remembered = what,
        Slot::Imprinted => ctx.imprinted = what,
    }
}

/// The line with one key under another name.
fn rename(p: &Params, from: &str, to: &str) -> Params {
    let body: Vec<String> = p
        .iter()
        .map(|(key, value)| format!("{}$ {value}", if key == from { to } else { key }))
        .collect();
    Params::parse(&body.join(" | "))
}

/// The step that makes the source remember `what`: objects or players.
fn remember_step(what: &Value, slot: Slot) -> Value {
    json!({"Remember": {"what": what, "slot": slot.name()}})
}

/// `DB$ Remember | Defined$ X` and `DB$ Imprint`: the slot gets the objects.
fn remember_api(ctx: &mut Ctx, p: &Params, slot: Slot) -> Tr<Vec<Value>> {
    let api = if slot == Slot::Remembered {
        "Remember"
    } else {
        "Imprint"
    };
    only(p, api, &["Defined"])?;
    let Some(word) = p.get("Defined") else {
        return unmapped(format!("key:{api}.no-Defined"));
    };
    let what = defined(ctx, word)?;
    set(ctx, slot, Some(what.clone()));
    if ctx.memory.crosses(slot) {
        return Ok(vec![remember_step(&what, slot)]);
    }
    Ok(Vec::new())
}

/// `ChosenColor`, `ChosenType` and `NamedCard` on a card whose controller chose as it entered: the predicates that read
/// that choice.
pub fn entered_with(ctx: &Ctx, prop: &str) -> Tr<Value> {
    let (made, pred) = match prop {
        "ChosenColor" => (ctx.memory.entered.color, "ChosenColor"),
        "ChosenType" => (ctx.memory.entered.subtype, "ChosenSubtype"),
        "NamedCard" => (ctx.memory.entered.name, "ChosenName"),
        _ => return unmapped(format!("valid:{prop}")),
    };
    if made {
        Ok(json!(pred))
    } else {
        unmapped(format!("valid:{prop}-without-a-choice"))
    }
}

/// The `ETBReplacement` part that asks for a card name: any card, or a nonland one.
pub fn name_choice(part: &Params) -> Tr<Value> {
    let kind = match part.get("ValidCards") {
        None | Some("Card") => "CardName",
        Some("Card.nonLand") => "NonlandCardName",
        Some(other) => return unmapped(format!("key:ETBReplacement.ValidCards={other}")),
    };
    super::ctx::check_keys(
        part,
        "ETBReplacement",
        &["DB", "Defined", "ValidCards"],
        |_, _| false,
    )?;
    Ok(json!({ "Choose": kind }))
}

/// What the `ETBReplacement` choices of the face make: a colour, a creature type, a card name.
fn entering(face: &Face, plan: &mut Plan) {
    for body in &face.keywords {
        let Some(rest) = body.strip_prefix("ETBReplacement:Other:") else {
            continue;
        };
        let name = rest.split(':').next().unwrap_or(rest);
        let Some(text) = face.svars.get(name) else {
            continue;
        };
        let part = Params::parse(text);
        match (part.get("DB"), part.get("Type")) {
            (Some("ChooseColor"), None) => plan.entered.color = true,
            (Some("ChooseType"), Some("Creature")) => plan.entered.subtype = true,
            (Some("NameCard"), _) => plan.entered.name = true,
            _ => {}
        }
    }
}

/// `Cleanup`: clearing a slot the card reads across lines forgets it; elsewhere it is bookkeeping.
fn cleanup(
    ctx: &mut Ctx,
    p: &Params,
    run: fn(&mut Ctx, &Params, &str) -> Tr<Vec<Value>>,
) -> Tr<Vec<Value>> {
    let mut out = run(ctx, p, "Cleanup")?;
    for (key, slot) in [
        ("ClearRemembered", Slot::Remembered),
        ("ClearImprinted", Slot::Imprinted),
    ] {
        if p.get(key) == Some("True") && ctx.memory.crosses(slot) {
            out.push(json!({"Forget": {"slot": slot.name()}}));
        }
    }
    Ok(out)
}
