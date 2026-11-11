//! What restricted mana may be spent on: the `RestrictValid$` of a Forge `Mana` ability read into the engine's list of uses.

use serde_json::{Value, json};

use super::ctx::{CARD_TYPES, Ctx, SUPERTYPES, Tr, unmapped};

/// The mana value `cmcGE4` asks for at least.
fn min_value(word: &str) -> Option<u32> {
    word.strip_prefix("cmcGE")?.parse().ok().filter(|n| *n > 0)
}

/// One property of a spell or ability source, as a part of the filter it is added to.
fn add_property(ctx: &Ctx, filter: &mut Value, word: &str, ability: bool) -> bool {
    let push = |filter: &mut Value, key: &str, item: Value| {
        let list = &mut filter[key];
        if let Some(items) = list.as_array_mut() {
            items.push(item);
        } else {
            *list = json!([item]);
        }
    };
    let fact = |filter: &mut Value, name: &str| push(filter, "facts", json!(name));
    match word {
        "MultiColor" => filter["multicolored"] = json!(true),
        "hasXCost" => fact(filter, "XInCost"),
        "sharesCreatureTypeWith Commander" => fact(filter, "SharesCommanderType"),
        "YouDontOwn" => fact(filter, "NotOwned"),
        "wasCastFromYourGraveyard" => fact(filter, "FromYourGraveyard"),
        "wasCastFromGraveyard" => fact(filter, "FromGraveyard"),
        "inZoneBattlefield" if ability => fact(filter, "OnBattlefield"),
        "Colorless" => fact(filter, "Colorless"),
        "wasCastFromExile" if !ability => fact(filter, "FromExile"),
        "!wasCastFromYourHand" if !ability => fact(filter, "NotFromHand"),
        "withDevoid" => fact(filter, "Devoid"),
        "withFlashback" => fact(filter, "Flashback"),
        // An outlaw is any one of the five creature types (CR 205.3m).
        "Outlaw" => {
            filter["subtypes"] = json!(["Assassin", "Mercenary", "Pirate", "Rogue", "Warlock"]);
        }
        _ if min_value(word).is_some() => filter["min_value"] = json!(min_value(word)),
        _ => {
            if let Some(rest) = word.strip_prefix("non")
                && CARD_TYPES.contains(&rest)
            {
                push(filter, "without", json!(rest));
            } else if CARD_TYPES.contains(&word) {
                push(filter, "types", json!(word));
            } else if SUPERTYPES.contains(&word) {
                push(filter, "supertypes", json!(word));
            } else if ["White", "Blue", "Black", "Red", "Green"].contains(&word) {
                push(filter, "colors", json!(word));
            } else if word.starts_with(char::is_uppercase) && ctx.world.subtypes.contains(word) {
                push(filter, "subtypes", json!(word));
            } else {
                return false;
            }
        }
    }
    true
}

/// A filter from `Creature+Dragon`-style properties; a second subtype or colour would be read as "or", so it is refused.
pub fn filter(ctx: &Ctx, properties: &str, ability: bool) -> Option<Value> {
    let mut filter = json!({});
    for word in properties.split('+').filter(|w| !w.is_empty()) {
        if !add_property(ctx, &mut filter, word, ability) {
            return None;
        }
    }
    let many = |key: &str| {
        filter
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|l| l.len() > 1)
    };
    // Outlaw stands alone for its five types; each fact must hold, so several are one filter.
    let outlaw = properties.split('+').any(|word| word == "Outlaw");
    let words = properties.split('+').filter(|w| !w.is_empty()).count();
    if (outlaw && words > 1) || (!outlaw && many("subtypes")) || many("colors") {
        return None;
    }
    Some(filter)
}

/// The uses a `RestrictValid$` lists, any one of which may be paid with the mana.
pub fn uses(ctx: &Ctx, restriction: &str) -> Tr<Vec<Value>> {
    let fail = || unmapped(format!("key:Mana.RestrictValid={restriction}"));
    match restriction {
        "Activated" => return Ok(vec![json!({"Ability": {}})]),
        "CostContainsX" => return Ok(vec![json!("CostWithX")]),
        // "This mana can't be spent to cast spells": abilities and every other cost.
        "nonSpell" => return Ok(vec![json!({"Ability": {}}), json!("OtherCosts")]),
        // "Spend this mana only to cast your commander".
        "Spell.IsCommander+YouOwn" => {
            return Ok(vec![json!({"Spell": {"facts": ["YourCommander"]}})]);
        }
        // "Can't be spent to cast nonartifact spells": anything else is allowed.
        "CantCastNonArtifactSpells" => {
            return Ok(vec![
                json!({"Spell": {"types": ["Artifact"]}}),
                json!({"Ability": {}}),
            ]);
        }
        // "This mana can't be spent to cast spells from your hand": spells from anywhere else and abilities.
        "CantCastSpellFromHand" => {
            return Ok(vec![
                json!({"Spell": {"facts": ["NotFromHand"]}}),
                json!({"Ability": {}}),
                json!("OtherCosts"),
            ]);
        }
        _ => {}
    }
    let mut out = Vec::new();
    for alternative in restriction.split(',') {
        // A special action names no kind: "to turn permanents face up", "unlock a door", "foretell a card".
        let special = match alternative {
            "Static.isTurnFaceUp" => Some("TurnFaceUp"),
            "Static.Unlock" => Some("Unlock"),
            "Static.Foretelling" => Some("Foretell"),
            _ => None,
        };
        if let Some(action) = special {
            out.push(json!({ "Special": action }));
            continue;
        }
        let (head, properties) = alternative.split_once('.').unwrap_or((alternative, ""));
        let ability = match head {
            "Spell" => false,
            "Activated" => true,
            _ => return fail(),
        };
        let Some(filter) = filter(ctx, properties, ability) else {
            return fail();
        };
        let mut use_ = serde_json::Map::new();
        use_.insert(if ability { "Ability" } else { "Spell" }.to_owned(), filter);
        out.push(Value::Object(use_));
    }
    Ok(out)
}

/// The effects a rider may run: each reads the spell by the triggering object, which stays one until the spell resolves.
const RIDER_EFFECTS: [&str; 6] = [
    "Scry",
    "CopySpell",
    "CreateToken",
    "Draw",
    "GainLife",
    "LoseLife",
];

/// `TriggersWhenSpent$ Name`: "when that mana is spent to cast a creature spell, scry 1", one rider per spell kind the
/// trigger names.
pub fn when_spent(ctx: &mut Ctx, name: &str) -> Tr<Vec<Value>> {
    let gap = |what: &str| unmapped(format!("key:Mana.TriggersWhenSpent-{what}"));
    let trigger = ctx.sub(name)?;
    if trigger.get("Mode") != Some("SpellCast") {
        return gap("mode");
    }
    let plain = trigger.iter().all(|(key, _)| {
        matches!(
            key,
            "Mode" | "ValidCard" | "ValidActivatingPlayer" | "Execute" | "TriggerDescription"
        )
    });
    if !plain
        || trigger
            .get("ValidActivatingPlayer")
            .is_some_and(|w| w != "You")
    {
        return gap("key");
    }
    let (Some(valid), Some(execute)) = (trigger.get("ValidCard"), trigger.get("Execute")) else {
        return gap("shape");
    };
    let mut filters = Vec::new();
    for alternative in valid.split(',') {
        let (head, properties) = alternative.split_once('.').unwrap_or((alternative, ""));
        let mut text = if head == "Card" {
            String::new()
        } else if CARD_TYPES.contains(&head) || ctx.world.subtypes.contains(head) {
            head.to_owned()
        } else {
            return gap("valid");
        };
        if !properties.is_empty() {
            if !text.is_empty() {
                text.push('+');
            }
            text.push_str(properties);
        }
        let Some(filter) = filter(ctx, &text, false) else {
            return gap("valid");
        };
        filters.push(filter);
    }
    let part = ctx.sub(execute)?;
    let effects = ctx.nested(None, |ctx| {
        ctx.trigger = Some(super::triggered::Shape {
            mode: "SpellCast".to_owned(),
            itself: false,
            by: false,
            to_player: false,
            pattern: "SpellCast".to_owned(),
            arrives: None,
            targeting: None,
            destination: None,
        });
        let effects = super::effects::chain_at(ctx, &part, false)?;
        if ctx.targets.is_empty() {
            Ok(effects)
        } else {
            gap("target")
        }
    })?;
    let known = effects.iter().all(|effect| {
        effect
            .as_object()
            .and_then(|map| map.keys().next())
            .is_some_and(|key| RIDER_EFFECTS.contains(&key.as_str()))
    });
    if !known {
        return gap("effect");
    }
    Ok(filters
        .into_iter()
        .map(|spell| json!({"When": {"spell": spell, "effects": effects}}))
        .collect())
}

/// The spells of an `AddsCounters$` (`Card.Creature+MultiColor_P1P1_1`) and the counters they enter with.
fn counters_bonus(ctx: &Ctx, counters: &str) -> Option<(Value, u32)> {
    let [valid, "P1P1", count] = counters.split('_').collect::<Vec<_>>()[..] else {
        return None;
    };
    let count = count.parse::<u32>().ok().filter(|count| *count > 0)?;
    let properties = match valid.split_once('.') {
        Some(("Card" | "Spell", rest)) => rest.to_owned(),
        Some(("Creature", rest)) => format!("Creature+{rest}"),
        None if valid == "Creature" => valid.to_owned(),
        _ => return None,
    };
    Some((filter(ctx, &properties, false)?, count))
}

/// `AddsCounters$`/`AddsKeywords$`: what a permanent spell gets when the mana is spent to cast it ("that creature enters
/// with an additional +1/+1 counter on it", "it gains haste until end of turn"); none without either key.
pub fn enters_bonus(ctx: &Ctx, p: &crate::forge::script::Params) -> Tr<Option<Value>> {
    let gap = |key: &str| unmapped(format!("key:Mana.{key}"));
    let mut bonus = serde_json::Map::new();
    let mut spell = None;
    if let Some(counters) = p.get("AddsCounters") {
        let Some((filter, count)) = counters_bonus(ctx, counters) else {
            return gap("AddsCounters");
        };
        spell = Some(filter);
        bonus.insert("counters".to_owned(), json!(count));
    }
    if let Some(keyword) = p.get("AddsKeywords") {
        // A keyword that acts as the permanent enters (riot) would come too late once the permanent has entered.
        if keyword != "Haste" || spell.is_some() {
            return gap("AddsKeywords");
        }
        let Some(valid) = p
            .get("AddsKeywordsValid")
            .and_then(|valid| valid.strip_prefix("Spell."))
        else {
            return gap("AddsKeywordsValid");
        };
        let Some(kind) = filter(ctx, valid, false) else {
            return gap("AddsKeywordsValid");
        };
        spell = Some(kind);
        bonus.insert("keyword".to_owned(), json!(keyword));
        match p.get("AddsKeywordsUntil") {
            None => {}
            Some("UntilEOT") => {
                bonus.insert("this_turn".to_owned(), json!(true));
            }
            Some(_) => return gap("AddsKeywordsUntil"),
        }
    } else if p.has("AddsKeywordsValid") || p.has("AddsKeywordsUntil") {
        return gap("AddsKeywords");
    }
    let Some(spell) = spell else {
        return Ok(None);
    };
    bonus.insert("spell".to_owned(), spell);
    Ok(Some(json!({ "Enters": bonus })))
}
