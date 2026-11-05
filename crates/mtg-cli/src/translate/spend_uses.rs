//! What restricted mana may be spent on: the `RestrictValid$` of a Forge `Mana` ability read into the engine's list of uses.

use serde_json::{Value, json};

use super::ctx::{CARD_TYPES, Ctx, SUPERTYPES, Tr, unmapped};

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
        "wasCastFromYourGraveyard" | "wasCastFromGraveyard" => fact(filter, "FromGraveyard"),
        "inZoneBattlefield" if ability => fact(filter, "OnBattlefield"),
        "cmcGE5" => filter["min_value"] = json!(5),
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
    let many = |key: &str| filter.get(key).and_then(Value::as_array).is_some_and(|l| l.len() > 1);
    if many("subtypes") || many("colors") || many("facts") {
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
        // "Can't be spent to cast nonartifact spells": anything else is allowed.
        "CantCastNonArtifactSpells" => {
            return Ok(vec![
                json!({"Spell": {"types": ["Artifact"]}}),
                json!({"Ability": {}}),
            ]);
        }
        _ => {}
    }
    let mut out = Vec::new();
    for alternative in restriction.split(',') {
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
