//! Keywords: Forge's `K:` lines and `KW$` words as the DSL's keywords.

use serde_json::{Value, json};

use super::amount::{amount, counter_kind};
use super::cost::cost;
use super::ctx::{Ctx, Tr, unmapped};
use super::effects::chain_at;
use super::valid::selector_any;
use crate::forge::script::Params;

mod champion;
mod equip;
mod extra;
mod more;

/// The DSL name of a keyword that has no parameter.
pub fn keyword_name(word: &str) -> Option<&'static str> {
    Some(match word.trim() {
        "Flying" => "Flying",
        "Reach" => "Reach",
        "First Strike" => "FirstStrike",
        "Double Strike" => "DoubleStrike",
        "Deathtouch" => "Deathtouch",
        "Trample" => "Trample",
        "Lifelink" => "Lifelink",
        "Vigilance" => "Vigilance",
        "Haste" => "Haste",
        "Menace" => "Menace",
        "Defender" => "Defender",
        "Indestructible" => "Indestructible",
        "Hexproof" => "Hexproof",
        "Shroud" => "Shroud",
        "Flash" => "Flash",
        "Prowess" => "Prowess",
        "Convoke" => "Convoke",
        "Delve" => "Delve",
        "Cascade" => "Cascade",
        "Storm" => "Storm",
        "Partner" => "Partner",
        "Battle cry" => "BattleCry",
        "Fuse" => "Fuse",
        "Living weapon" | "Living Weapon" => "LivingWeapon",
        "Changeling" => "Changeling",
        "Devoid" => "Devoid",
        "Infect" => "Infect",
        "Myriad" => "Myriad",
        "Daybound" => "Daybound",
        "Nightbound" => "Nightbound",
        "Wither" => "Wither",
        "Fear" => "Fear",
        "Intimidate" => "Intimidate",
        "Skulk" => "Skulk",
        "Shadow" => "Shadow",
        "Split second" => "SplitSecond",
        "Undaunted" => "Undaunted",
        "Rebound" => "Rebound",
        "Aftermath" => "Aftermath",
        "Undying" => "Undying",
        "Persist" => "Persist",
        "Exalted" => "Exalted",
        "Melee" => "Melee",
        "Mentor" => "Mentor",
        "Dethrone" => "Dethrone",
        "Extort" => "Extort",
        "Evolve" => "Evolve",
        "Ravenous" => "Ravenous",
        "Riot" => "Riot",
        "Decayed" => "Decayed",
        "Jump-start" => "JumpStart",
        "Retrace" => "Retrace",
        "Improvise" => "Improvise",
        "Ascend" => "Ascend",
        "Horsemanship" => "Horsemanship",
        "Phasing" => "Phasing",
        "Cipher" => "Cipher",
        "Exploit" => "Exploit",
        "Soulbond" => "Soulbond",
        "Gravestorm" => "Gravestorm",
        "Assist" => "Assist",
        "Sunburst" => "Sunburst",
        "Demonstrate" => "Demonstrate",
        "Read ahead" => "ReadAhead",
        "Umbra armor" => "UmbraArmor",
        "Bargain" => "Bargain",
        "Conspire" => "Conspire",
        "Choose a Background" => "ChooseABackground",
        "Doctor's companion" => "DoctorsCompanion",
        "For Mirrodin" | "For Mirrodin!" => "ForMirrodin",
        "Job select" => "JobSelect",
        _ => return None,
    })
}

/// Keywords with one cost: the Forge word and the DSL variant.
const COSTED: [(&str, &str); 43] = [
    ("Mayhem", "Mayhem"),
    ("Disturb", "Disturb"),
    ("Harmonize", "Harmonize"),
    ("Offspring", "Offspring"),
    ("Escape", "Escape"),
    ("Equip", "Equip"),
    ("Ward", "Ward"),
    ("Reconfigure", "Reconfigure"),
    ("Cycling", "Cycling"),
    ("Kicker", "Kicker"),
    ("Multikicker", "Multikicker"),
    ("Buyback", "Buyback"),
    ("Entwine", "Entwine"),
    ("Flashback", "Flashback"),
    ("Madness", "Madness"),
    ("Miracle", "Miracle"),
    ("Morph", "Morph"),
    ("Megamorph", "Megamorph"),
    ("Disguise", "Disguise"),
    ("Evoke", "Evoke"),
    ("Overload", "Overload"),
    ("Mutate", "Mutate"),
    ("Encore", "Encore"),
    ("Squad", "Squad"),
    ("Foretell", "Foretell"),
    ("Plot", "Plot"),
    ("Unearth", "Unearth"),
    ("Dash", "Dash"),
    ("Bestow", "Bestow"),
    ("Blitz", "Blitz"),
    ("Prowl", "Prowl"),
    ("Ninjutsu", "Ninjutsu"),
    ("Echo", "Echo"),
    ("Cumulative upkeep", "CumulativeUpkeep"),
    ("Level up", "LevelUp"),
    ("Outlast", "Outlast"),
    ("Scavenge", "Scavenge"),
    ("Transmute", "Transmute"),
    ("Embalm", "Embalm"),
    ("Eternalize", "Eternalize"),
    ("Spectacle", "Spectacle"),
    ("Replicate", "Replicate"),
    ("Emerge", "Emerge"),
];

/// Keywords with one number.
const NUMBERED: [(&str, &str); 14] = [
    ("Crew", "Crew"),
    ("Saddle", "Saddle"),
    ("Dredge", "Dredge"),
    ("Annihilator", "Annihilator"),
    ("Toxic", "Toxic"),
    ("Afterlife", "Afterlife"),
    ("Fabricate", "Fabricate"),
    ("Afflict", "Afflict"),
    ("Vanishing", "Vanishing"),
    ("Fading", "Fading"),
    ("Graft", "Graft"),
    ("Renown", "Renown"),
    ("Casualty", "Casualty"),
    ("Hideaway", "Hideaway"),
];

/// The DSL keyword a `K:` line or a `KW$` word stands for.
pub fn keyword_value(ctx: &Ctx, text: &str) -> Tr<Value> {
    let text = text.trim();
    if let Some(name) = keyword_name(text) {
        return Ok(json!(name));
    }
    if let Some(found) = more::special(ctx, text) {
        return found;
    }
    let mut fields = text.split(':');
    let name = fields.next().unwrap_or_default();
    let first = fields.next();
    if let (Some(&(_, variant)), Some(first)) =
        (COSTED.iter().find(|(word, _)| *word == name), first)
    {
        return costed(ctx, name, variant, first, fields.next());
    }
    if let (Some(&(_, variant)), Some(number)) =
        (NUMBERED.iter().find(|(word, _)| *word == name), first)
        && let Ok(number) = number.parse::<u32>()
    {
        return Ok(json!({ variant: number }));
    }
    let second = fields.next();
    if mtg_cards::macros::is_keyword(name) {
        return macro_keyword(name, first, second, fields.next());
    }
    other_keyword(ctx, name, first, second)
}

/// A keyword of the macro table (`mtg_cards::macros`): its number when it prints one. A number that is not a plain
/// count (`X`, `Sunburst`), a word after it or a number the keyword does not take stays a gap.
fn macro_keyword(
    name: &str,
    first: Option<&str>,
    second: Option<&str>,
    third: Option<&str>,
) -> Tr<Value> {
    let lower = name.to_lowercase();
    let amount = match (mtg_cards::macros::takes_number(&lower), first) {
        (true, Some(number)) => number.parse::<u32>().ok().filter(|number| *number > 0),
        (false, None) => Some(0),
        _ => None,
    };
    match amount {
        Some(amount) if second.is_none() && third.is_none() => {
            Ok(json!({"Macro": {"name": lower, "amount": amount}}))
        }
        _ => unmapped(format!("keyword:{name}-shape")),
    }
}

/// A keyword with a cost; a second field (a restriction or a description) is not read.
fn costed(ctx: &Ctx, name: &str, variant: &str, cost_text: &str, rest: Option<&str>) -> Tr<Value> {
    if rest.is_some() && matches!(name, "Equip" | "Kicker" | "Escape" | "Ninjutsu") {
        return unmapped(format!("keyword:{name}-with-restriction"));
    }
    // A reduction in the second field changes the cost the keyword is paid at: dropping it would print the wrong cost.
    if rest.is_some_and(|text| text.contains("ReduceCost$")) {
        return unmapped(format!("keyword:{name}-with-reduction"));
    }
    Ok(json!({ variant: cost(ctx, cost_text)? }))
}

fn other_keyword(ctx: &Ctx, name: &str, first: Option<&str>, second: Option<&str>) -> Tr<Value> {
    match (name, first) {
        ("Protection", Some(valid)) => Ok(json!({"Protection": selector_any(ctx, valid)?})),
        // "Hexproof from triggered abilities" is not a quality of the source.
        ("Hexproof", Some("Activated" | "Triggered")) => unmapped("keyword:Hexproof"),
        ("Hexproof", Some(valid)) => {
            let colored = ["White", "Blue", "Black", "Red", "Green"].contains(&valid);
            let spec = if colored {
                format!("Card.{valid}")
            } else {
                valid.to_owned()
            };
            Ok(json!({"HexproofFrom": selector_any(ctx, &spec)?}))
        }
        ("Enchant", Some(valid)) if !matches!(valid, "Player" | "Opponent") => {
            Ok(json!({"Enchant": selector_any(ctx, valid)?}))
        }
        ("Enchant", Some("Player")) => Ok(json!({"EnchantPlayer": {}})),
        ("Enchant", Some("Opponent")) => Ok(json!({"EnchantPlayer": {"opponent": true}})),
        ("Suspend", Some(time)) => match (time.parse::<u32>(), second) {
            (Ok(time), Some(cost_text)) => {
                Ok(json!({"Suspend": {"time": time, "cost": cost(ctx, cost_text)?}}))
            }
            _ => unmapped("keyword:Suspend"),
        },
        ("Partner", Some(group)) => Ok(json!({"PartnerGroup": group})),
        ("Partner with", Some(with)) => Ok(json!({"PartnerWith": with})),
        ("Landwalk", Some(land)) => Ok(json!({"Landwalk": selector_any(ctx, land)?})),
        (word, None) if word.ends_with("walk") => landwalk(word),
        _ => unmapped(format!("keyword:{name}")),
    }
}

fn landwalk(word: &str) -> Tr<Value> {
    let land = match word {
        "Plainswalk" => "Plains",
        "Islandwalk" => "Island",
        "Swampwalk" => "Swamp",
        "Mountainwalk" => "Mountain",
        "Forestwalk" => "Forest",
        _ => return unmapped(format!("keyword:{word}")),
    };
    Ok(json!({"Landwalk": [{"Subtype": land}]}))
}

/// Rules about building a deck: they belong to deck validation and change nothing about play.
const DECK_RULES: [&str; 3] = [
    "A deck can have any number of cards named CARDNAME.",
    "Your minimum deck size is reduced by five.",
    "Your deck can't have basic land cards.",
];

/// The abilities a `K:` line stands for: those of `keyword_ability`, or the chapters of a Saga.
pub fn keyword_abilities(ctx: &mut Ctx, body: &str) -> Vec<Tr<Value>> {
    // The costs of the modes are the modal spell's own: the spell line reads them.
    if matches!(body.trim(), "Spree" | "Tiered") || body.trim().starts_with("Escalate:") {
        return Vec::new();
    }
    if DECK_RULES.contains(&body.trim()) {
        return Vec::new();
    }
    if let Some(quirks) = combat_quirks(body.trim()) {
        return quirks.iter().map(|quirk| Ok(combat_rule(quirk))).collect();
    }
    if let Some(rules) = super::keyword_prevent::prevent_keyword(body) {
        return rules.into_iter().map(Ok).collect();
    }
    if let Some(levels) = super::keyword_class::class_level(ctx, body) {
        return levels;
    }
    if let Some(forms) = super::effects_r2h::keyword_forms(ctx, body) {
        return forms;
    }
    if let Some(types) = body.strip_prefix("Champion:") {
        return vec![champion::champion(ctx, types)];
    }
    if body.starts_with("Equip:") && body.contains('$') {
        let fields: Vec<&str> = body.split(':').collect();
        return equip::equip_with(ctx, &fields);
    }
    match body.strip_prefix("Chapter:") {
        Some(rest) => chapters(ctx, rest),
        None => super::keyword_alt::alt_keyword(body)
            .unwrap_or_else(|| vec![super::card::audited_one(|| keyword_ability(ctx, body))]),
    }
}

/// The combat rules a keyword line in words stands for: the creature itself, in the words Forge prints.
fn combat_quirks(body: &str) -> Option<Vec<Value>> {
    Some(match body {
        "CARDNAME can't attack alone." => vec![json!("CantAttackAlone")],
        "CARDNAME can't block alone." => vec![json!("CantBlockAlone")],
        "CARDNAME can't attack or block alone." => {
            vec![json!("CantAttackAlone"), json!("CantBlockAlone")]
        }
        "You may choose not to untap CARDNAME during your untap step." => {
            vec![json!("MayStayTapped")]
        }
        "CARDNAME must be blocked if able." => {
            vec![json!({"MustBeBlocked": "AtLeastOne"})]
        }
        "All creatures able to block CARDNAME do so." | "MustBeBlockedByAll" => {
            vec![json!({"MustBeBlocked": "Every"})]
        }
        _ => return None,
    })
}

/// The static ability that gives the creature itself a combat rule.
fn combat_rule(quirk: &Value) -> Value {
    json!({"Rule": {"rule": {"Combat": {"who": "This", "quirk": quirk}}}})
}

/// `Chapter:3:First,Second,Third`: a triggered ability for each chapter, from the part the chapter names.
fn chapters(ctx: &mut Ctx, rest: &str) -> Vec<Tr<Value>> {
    let mut fields = rest.split(':');
    let count = fields.next().and_then(|count| count.parse::<usize>().ok());
    let names: Vec<&str> = fields.next().unwrap_or_default().split(',').collect();
    if count != Some(names.len()) || fields.next().is_some() {
        return vec![unmapped("keyword:Chapter-shape")];
    }
    names
        .iter()
        .enumerate()
        .map(|(at, name)| super::card::audited_one(|| chapter(ctx, at + 1, name)))
        .collect()
}

fn chapter(ctx: &mut Ctx, number: usize, name: &str) -> Tr<Value> {
    ctx.reset();
    let part = ctx.sub(name)?;
    let effects = chain_at(ctx, &part, false)?;
    let mut ability = json!({"when": {"Chapter": {"number": number}}, "effects": effects});
    if !ctx.targets.is_empty() {
        ability["targets"] = Value::Array(ctx.targets.clone());
    }
    Ok(json!({ "Triggered": ability }))
}

/// The ability a `K:` line stands for: a keyword, or the replacement of a permanent that enters with counters or makes a
/// choice (`etbCounter:P1P1:2`, `ETBReplacement:Other:ChooseColor`).
pub fn keyword_ability(ctx: &mut Ctx, body: &str) -> Tr<Value> {
    ctx.reset();
    let fields: Vec<&str> = body.split(':').collect();
    if let ["AlternateAdditionalCost", first, second] = fields.as_slice() {
        let costs = [cost(ctx, first)?, cost(ctx, second)?];
        return Ok(json!({"AdditionalCost": {"cost": [{"OneOf": costs}], "required": true}}));
    }
    if let ["MayEffectFromOpeningHand", "FromHand"] = fields.as_slice() {
        return opening_hand(ctx);
    }
    let mut condition = None;
    let by = match fields.as_slice() {
        ["etbCounter", kind, count, rest @ ..] if rest.len() <= 2 => {
            let Some(kind) = counter_kind(kind) else {
                return unmapped("keyword:etbCounter-kind");
            };
            condition = etb_condition(ctx, rest.first().copied())?;
            json!({"WithCounters": {"counter": kind, "count": amount(ctx, count)?}})
        }
        ["ETBReplacement", "Other", name] => choice(ctx, name)?,
        ["ETBReplacement", "Other", name, "Mandatory", "", valid]
            if valid.starts_with("Card.Self+") =>
        {
            return super::enters_with::self_enters(ctx, name, valid);
        }
        ["ETBReplacement", "Other", name, "Mandatory", zone, valid] if !valid.contains("Self") => {
            return extra::others(ctx, name, zone, valid);
        }
        ["ETBReplacement", "Copy", name, "Optional"] => {
            super::effects_copy::enters_as_copy(ctx, name)?
        }
        [
            "ETBReplacement",
            "Other",
            name,
            "Mandatory",
            zone,
            valid,
            ..,
        ] => {
            return super::effects_r2h::extra_counters(ctx, name, zone, valid);
        }
        [name, ..] if matches!(*name, "etbCounter" | "ETBReplacement") => {
            return unmapped(format!("keyword:{name}-shape"));
        }
        _ => return Ok(json!({"Keyword": keyword_value(ctx, body)?})),
    };
    let mut replacement = json!({"when": {"Enters": {"what": ["This"]}}, "by": by});
    if let Some(condition) = condition {
        replacement["condition"] = condition;
    }
    Ok(json!({ "Replacement": replacement }))
}

/// `MayEffectFromOpeningHand:FromHand`: the card may begin the game on the battlefield, when its part says exactly that.
fn opening_hand(ctx: &Ctx) -> Tr<Value> {
    let Some(text) = ctx.svar("FromHand") else {
        return unmapped("svar:missing");
    };
    let part = Params::parse(text);
    let exact = part.get("DB") == Some("ChangeZone")
        && part.get("Defined") == Some("Self")
        && part.get("Origin") == Some("Hand")
        && part.get("Destination") == Some("Battlefield");
    if !exact {
        return unmapped("keyword:MayEffectFromOpeningHand-shape");
    }
    super::ctx::check_keys(
        &part,
        "MayEffectFromOpeningHand",
        &["DB", "Defined", "Origin", "Destination"],
        |_, _| false,
    )?;
    Ok(json!("OpeningHand"))
}

/// What the part named by an `ETBReplacement` makes the permanent's controller choose as it enters.
fn choice(ctx: &Ctx, name: &str) -> Tr<Value> {
    let Some(text) = ctx.svar(name) else {
        return unmapped("svar:missing");
    };
    let part = Params::parse(text);
    // The chooser is the controller as the permanent enters.
    if part.get("Defined").is_some_and(|who| who != "You") {
        return unmapped("key:ETBReplacement.Defined");
    }
    let hints_only = |read: &[&str]| -> Tr<()> {
        super::ctx::check_keys(&part, "ETBReplacement", read, |_, _| false)
    };
    match (part.get("DB"), part.get("Type"), part.get("Choices")) {
        (Some("ChooseColor"), None, None) => {
            hints_only(&["DB", "Defined", "Exclude"])?;
            match part.get("Exclude") {
                None => Ok(json!({"Choose": "Color"})),
                Some(color) => match excluded_color(color) {
                    Some(color) => Ok(json!({"Choose": {"ColorOtherThan": color}})),
                    None => unmapped("key:ETBReplacement.Exclude"),
                },
            }
        }
        (Some("ChooseType"), Some("Creature"), None) => {
            hints_only(&["DB", "Defined", "Type"])?;
            Ok(json!({"Choose": "CreatureType"}))
        }
        (Some("ChoosePlayer"), None, Some(choices)) => {
            hints_only(&["DB", "Defined", "Choices"])?;
            match choices {
                "Player.Opponent" => Ok(json!({"Choose": "Opponent"})),
                "Player" => Ok(json!({"Choose": "Player"})),
                other => unmapped(format!("key:ETBReplacement.Choices={other}")),
            }
        }
        (Some("NameCard"), _, None) => super::memory::name_choice(&part),
        (Some("AlterAttribute"), _, _) => extra::prepared(&part),
        (api, _, _) => unmapped(format!("keyword:ETBReplacement-{}", api.unwrap_or("?"))),
    }
}

/// The DSL name of the color `Exclude$` names.
fn excluded_color(word: &str) -> Option<&'static str> {
    ["white", "blue", "black", "red", "green"]
        .iter()
        .position(|known| *known == word)
        .map(|at| ["White", "Blue", "Black", "Red", "Green"][at])
}

/// The condition field of an `etbCounter` line (the field after the count; the next one is the description): none for
/// no field or `no Condition`, else the escape check or a `CheckSVar` condition; anything else is a gap.
fn etb_condition(ctx: &Ctx, field: Option<&str>) -> Tr<Option<Value>> {
    let Some(field) = field else {
        return Ok(None);
    };
    match field {
        "no Condition" => return Ok(None),
        "CheckSVar$ WasKicked" => return Ok(Some(json!("Kicked"))),
        "ValidCard$ Card.Self+escaped" => return Ok(Some(json!({"CastWith": "Escape"}))),
        "Revolt$ True" => return Ok(Some(super::condition::revolt())),
        _ => {}
    }
    if let Some(color) = field.strip_prefix("Adamant$ ") {
        return super::condition::adamant(color).map(Some);
    }
    if let Some(spec) = field.strip_prefix("ValidCard$ Card.Self+") {
        if spec.starts_with("wasCast") {
            let cast = super::condition::self_was_cast(ctx, &format!("Card.Self+{spec}"))?;
            return Ok(Some(cast));
        }
        return unmapped("keyword:etbCounter-condition");
    }
    let params = Params::parse(field);
    if !params.has("SVarCompare")
        && let Some(prop) = params
            .get("CheckSVar")
            .and_then(|name| ctx.svar(name))
            .and_then(|text| text.strip_prefix("Count$wasCast"))
            .and_then(|text| text.strip_suffix(".1.0"))
    {
        let cast = super::condition::self_was_cast(ctx, &format!("Card.Self+wasCast{prop}"))?;
        return Ok(Some(cast));
    }
    if !params.has("CheckSVar") {
        return unmapped("keyword:etbCounter-condition");
    }
    let keys = ["CheckSVar", "SVarCompare"];
    super::ctx::check_keys(&params, "etbCounter", &keys, |_, _| false)?;
    match super::condition::condition(ctx, &params)? {
        Some(condition) => Ok(Some(condition)),
        None => unmapped("keyword:etbCounter-condition"),
    }
}

/// The keywords of a `KW$` value, separated by ` & `.
pub fn keyword_list(ctx: &Ctx, text: &str) -> Tr<Vec<Value>> {
    text.split('&')
        .map(|word| keyword_value(ctx, word))
        .collect()
}
