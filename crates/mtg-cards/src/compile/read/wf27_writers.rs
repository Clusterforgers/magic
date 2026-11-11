//! wf27 writers for the shapes drafts use most: maximum hand sizes, the top card of each library revealed, a creature
//! that crews as though stronger, a life floor, landwalk ignored, lands made a basic land type, permanents that don't
//! untap, a card name chosen as a permanent enters, damage to or by a creature prevented, a cast in place of the mana
//! cost under more conditions, colorless and extra blocks until end of turn, and the basic land type of your choice.

use serde_json::{Value, json};

use super::amounts::count_word;
use crate::ability::Ability;
use crate::value::Condition;

/// The JSON of an ability.
fn value_of(ability: &Ability) -> Option<Value> {
    serde_json::to_value(ability).ok()
}

/// The ability of a JSON value.
fn ability_of(value: Value) -> Option<Ability> {
    serde_json::from_value(value).ok()
}

/// "one", "two", ... for a small number, else its digits.
fn number(value: &Value) -> Option<String> {
    let n = u32::try_from(value.as_u64()?).ok()?;
    Some(count_word(n).map_or_else(|| n.to_string(), str::to_owned))
}

/// "a" or "an" before a word.
fn article(word: &str) -> &'static str {
    if word.starts_with(['A', 'E', 'I', 'O', 'U', 'a', 'e', 'i', 'o', 'u']) || word.starts_with("hour") {
        "an"
    } else {
        "a"
    }
}

/// The words of a counter kind: "+1/+1", "depletion".
fn counter_word(kind: &Value) -> Option<String> {
    match kind {
        Value::String(name) => Some(name.clone()),
        Value::Object(map) => map.get("Other")?.as_str().map(str::to_owned),
        _ => None,
    }
}

/// Adds the wordings of the wf27 writers.
pub(super) fn wf27_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Some(value) = value_of(ability) else {
        return;
    };
    let mut found = Vec::new();
    found.extend(rule_texts(&value));
    found.extend(land_type_texts(&value));
    found.extend(untap_texts(&value));
    found.extend(every_type_texts(&value, written));
    found.extend(defending_texts(&value, written));
    found.extend(card_name_texts(&value, written));
    found.extend(prevent_texts(&value));
    found.extend(colorless_texts(&value, written));
    found.extend(blocks_texts(&value, written));
    found.extend(land_type_choice_texts(&value, written));
    found.extend(cast_texts(&value));
    for text in found {
        let text = format!("{}.", text.trim_end_matches('.'));
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// The game rules: "Your maximum hand size is eight.", "Each opponent's maximum hand size is reduced by one.", "Players
/// play with the top card of their libraries revealed.", "~ saddles Mounts and crews Vehicles as though its power were 2
/// greater.", "Damage that would reduce your life total to less than 1 reduces it to 1 instead.", "Creatures with
/// islandwalk can be blocked as though they didn't have islandwalk.", "All creatures able to block enchanted creature do
/// so."
fn rule_texts(value: &Value) -> Vec<String> {
    let Some(body) = value.get("Rule").and_then(Value::as_object) else {
        return Vec::new();
    };
    // "As long as you have 7 or more life, damage that would reduce ...": the rule while the condition holds.
    if let Some(condition) = body.get("condition")
        && body.len() == 2
        && let Ok(held) = serde_json::from_value::<Condition>(condition.clone())
    {
        let plain = json!({ "Rule": { "rule": body.get("rule") } });
        let rules = rule_texts(&plain);
        if !rule_texts_conditional(&plain) {
            return Vec::new();
        }
        let mut texts = Vec::new();
        for rule in &rules {
            let rule = lowered(rule);
            for held in condition_texts(&held) {
                texts.push(format!("As long as {held}, {rule}"));
            }
            for held in super::statics::control_texts(&held) {
                texts.push(format!("If {held}, {rule}"));
            }
        }
        return texts;
    }
    if body.len() != 1 {
        return Vec::new();
    }
    let Some(rule) = body.get("rule") else {
        return Vec::new();
    };
    let whose = |who: &Value| match who.as_str() {
        Some("You") => Some("Your"),
        Some("Opponents") => Some("Each opponent's"),
        _ => None,
    };
    if let Some(held) = rule.get("MaxHandSize") {
        let (Some(whose), Some(size)) = (
            held.get("who").and_then(whose),
            held.get("size").and_then(number),
        ) else {
            return Vec::new();
        };
        return vec![format!("{whose} maximum hand size is {size}")];
    }
    if let Some(held) = rule.get("HandSizeChange") {
        let (Some(whose), Some(by)) = (
            held.get("who").and_then(whose),
            held.get("by").and_then(Value::as_i64),
        ) else {
            return Vec::new();
        };
        let way = if by > 0 { "increased" } else { "reduced" };
        let Some(size) = number(&json!(by.unsigned_abs())) else {
            return Vec::new();
        };
        if by == 0 {
            return Vec::new();
        }
        return vec![format!("{whose} maximum hand size is {way} by {size}")];
    }
    if rule == &json!({ "RevealTop": "EachPlayer" }) {
        return vec!["Players play with the top card of their libraries revealed".to_owned()];
    }
    if let Some(held) = rule.get("LifeFloor") {
        if held.get("who") != Some(&json!("You")) {
            return Vec::new();
        }
        let Some(floor) = held.get("floor").and_then(Value::as_u64) else {
            return Vec::new();
        };
        return vec![format!(
            "Damage that would reduce your life total to less than {floor} reduces it to {floor} instead"
        )];
    }
    let Some(combat) = rule.get("Combat") else {
        return Vec::new();
    };
    let (Some(who), Some(quirk)) = (combat.get("who"), combat.get("quirk")) else {
        return Vec::new();
    };
    if who == "This"
        && let Some(more) = quirk
            .get("TapPower")
            .and_then(|power| power.get("Plus"))
            .and_then(Value::as_u64)
    {
        return vec![format!(
            "~ saddles Mounts and crews Vehicles as though its power were {more} greater"
        )];
    }
    if let Some(walk) = quirk.get("IgnoreLandwalk")
        && who == &json!({ "Each": [{ "HasKeyword": { "Landwalk": walk } }] })
        && let [kind] = walk.as_array().map_or(&[][..], Vec::as_slice)
        && let Some(land) = kind.get("Subtype").and_then(Value::as_str)
    {
        let land = land.to_lowercase();
        return vec![format!(
            "Creatures with {land}walk can be blocked as though they didn't have {land}walk"
        )];
    }
    if who == "Attached" && quirk == &json!({ "MustBeBlocked": "Every" }) {
        return ["enchanted creature", "equipped creature"]
            .iter()
            .map(|holder| format!("All creatures able to block {holder} do so"))
            .collect();
    }
    Vec::new()
}

/// Whether a rule of `rule_texts` may hold under a condition: the life floor.
fn rule_texts_conditional(plain: &Value) -> bool {
    plain["Rule"]["rule"].get("LifeFloor").is_some()
}

/// The text with its first letter in lower case.
fn lowered(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_lowercase().chain(chars).collect())
        .unwrap_or_default()
}

/// The basic land types of an enchanted land: "Enchanted land is an Island.", "Enchanted land is a Mountain, Forest,
/// and Plains.", "Enchanted land is the chosen type."
fn land_type_texts(value: &Value) -> Vec<String> {
    let Some(body) = value.get("Static").and_then(Value::as_object) else {
        return Vec::new();
    };
    if body.len() != 2 || body.get("affected") != Some(&json!("Attached")) {
        return Vec::new();
    }
    let Some(mods) = body.get("mods").and_then(Value::as_array) else {
        return Vec::new();
    };
    if mods.as_slice() == [json!({ "SetLandTypes": [] }), json!("AddChosenSubtype")] {
        return vec!["Enchanted land is the chosen type".to_owned()];
    }
    let [only] = mods.as_slice() else {
        return Vec::new();
    };
    let Some(lands) = only.get("SetLandTypes").and_then(Value::as_array) else {
        return Vec::new();
    };
    let names: Vec<&str> = lands.iter().filter_map(Value::as_str).collect();
    if names.len() != lands.len() {
        return Vec::new();
    }
    let said = match names.as_slice() {
        [] => return Vec::new(),
        [one] => format!("{} {one}", article(one)),
        [first, second] => format!("{} {first} {second}", article(first)),
        [rest @ .., last] => format!("{} {}, and {last}", article(rest[0]), rest.join(", ")),
    };
    vec![format!("Enchanted land is {said}")]
}

/// Permanents that don't untap: "~ doesn't untap during your untap step if it has a depletion counter on it.",
/// "Islands don't untap during their controllers' untap steps.", "Enchanted creature doesn't untap during its
/// controller's untap step if it has a sleep counter on it."
fn untap_texts(value: &Value) -> Vec<String> {
    let Some(body) = value.get("Static").and_then(Value::as_object) else {
        return Vec::new();
    };
    if body.get("mods") != Some(&json!(["DoesntUntap"])) {
        return Vec::new();
    }
    let Some(affected) = body.get("affected") else {
        return Vec::new();
    };
    let condition = body.get("condition");
    if body.len() != 2 + usize::from(condition.is_some()) {
        return Vec::new();
    }
    let counter_on = |preds: &[Value]| -> Option<String> {
        let [held] = preds else {
            return None;
        };
        let kind = counter_word(held.get("HasCounter")?)?;
        Some(format!("{} {kind} counter", article(&kind)))
    };
    if let Some(condition) = condition {
        let subject = match affected.as_str() {
            Some("This") => "~ doesn't untap during your untap step",
            Some("Attached") => "Enchanted creature doesn't untap during its controller's untap step",
            _ => return Vec::new(),
        };
        // "if this Aura has a net counter on it": the Aura itself counts its counters.
        // Cocoon enchants a creature you control: its controller's untap step is yours.
        if affected == "Attached"
            && let Some(counted) = condition_counted_on_this(condition)
        {
            return [
                subject.to_owned(),
                subject.replace("its controller's untap step", "your untap step"),
            ]
            .into_iter()
            .map(|subject| format!("{subject} if this Aura has {counted} on it"))
            .collect();
        }
        let Ok(held) = serde_json::from_value::<Condition>(condition.clone()) else {
            return Vec::new();
        };
        let mut texts = Vec::new();
        for held in condition_texts(&held) {
            // "you control a snow permanent other than ~" is "you control another snow permanent".
            let another = held
                .strip_prefix("you control ")
                .and_then(|rest| rest.strip_suffix(" other than ~"))
                .and_then(|kind| kind.split_once(' '))
                .filter(|(article, _)| matches!(*article, "a" | "an"))
                .map(|(_, kind)| format!("you control another {kind}"));
            for held in std::iter::once(held).chain(another) {
                texts.push(format!("{subject} if {held}"));
                texts.push(format!("As long as {held}, {}", lowered(subject)));
            }
        }
        return texts;
    }
    let Some(preds) = affected.get("Each").and_then(Value::as_array) else {
        return Vec::new();
    };
    match preds.as_slice() {
        [this, rest @ ..] if this == "This" => counter_on(rest)
            .map(|counter| {
                vec![format!(
                    "~ doesn't untap during your untap step if it has {counter} on it"
                )]
            })
            .unwrap_or_default(),
        [creature, attached, rest @ ..]
            if creature == &json!({ "Type": "Creature" }) && attached == &json!({ "Is": "Attached" }) =>
        {
            counter_on(rest)
                .map(|counter| {
                    vec![format!(
                        "Enchanted creature doesn't untap during its controller's untap step if it has {counter} on it"
                    )]
                })
                .unwrap_or_default()
        }
        [held] if held.get("HasCounter").is_some() => counter_on(preds)
            .map(|counter| {
                vec![format!(
                    "Each permanent with {counter} on it doesn't untap during its controller's untap step"
                )]
            })
            .unwrap_or_default(),
        _ => {
            let Ok(selector) = serde_json::from_value::<crate::refs::Selector>(json!(preds)) else {
                return Vec::new();
            };
            if selector.0.iter().any(|pred| {
                matches!(
                    pred,
                    crate::refs::Pred::This | crate::refs::Pred::Is(_) | crate::refs::Pred::In(_)
                )
            }) {
                return Vec::new();
            }
            super::selectors::phrase_text(&selector, super::selectors::Form::All)
                .map(|group| {
                    let group = upper_first(&group);
                    vec![format!(
                        "{group} don't untap during their controllers' untap steps"
                    )]
                })
                .unwrap_or_default()
        }
    }
}

/// "a net counter" when the condition counts a counter of the kind on the card itself, one or more.
fn condition_counted_on_this(condition: &Value) -> Option<String> {
    let compare = condition.get("Compare")?;
    if compare.get("op")? != "Ge" || compare.get("right")? != &json!(1) {
        return None;
    }
    let counted = compare.get("left")?.get("Count")?.as_array()?;
    let [this, held] = counted.as_slice() else {
        return None;
    };
    (this == "This").then_some(())?;
    let kind = counter_word(held.get("HasCounter")?)?;
    Some(format!("{} {kind} counter", article(&kind)))
}

/// The text with its first letter in upper case.
fn upper_first(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// The wordings of a condition after "if", as the other writers say it.
pub(super) fn condition_texts(condition: &Condition) -> Vec<String> {
    let mut texts = super::as_long_as::texts(condition);
    for text in super::conditional::condition_texts(condition) {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
    texts
}

/// "Equipped creature gets +3/+0 and is every creature type.": an attached creature's bonus and every creature type,
/// written from the twin without the types.
fn every_type_texts(value: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Some(body) = value.get("Static").and_then(Value::as_object) else {
        return Vec::new();
    };
    if body.get("affected") != Some(&json!("Attached")) || body.contains_key("condition") {
        return Vec::new();
    }
    let Some(mods) = body.get("mods").and_then(Value::as_array) else {
        return Vec::new();
    };
    let Some((last, rest)) = mods.split_last() else {
        return Vec::new();
    };
    if last != "AddAllCreatureTypes" || rest.is_empty() || rest.contains(last) {
        return Vec::new();
    }
    let mut plain = value.clone();
    plain["Static"]["mods"] = json!(rest);
    let Some(plain) = ability_of(plain) else {
        return Vec::new();
    };
    let mut texts = Vec::new();
    for text in written(&plain) {
        let text = text.trim_end_matches('.');
        if !text.contains(" gets ") {
            continue;
        }
        for every in ["every creature type", "all creature types"] {
            // "gets +2/+2 and has reach" becomes "gets +2/+2, has reach, and is every creature type".
            let said = match text.split_once(" and has ") {
                Some((gets, has)) => format!("{gets}, has {has}, and is {every}"),
                None => format!("{text} and is {every}"),
            };
            texts.push(said);
        }
    }
    texts
}

/// A static that holds while defending player controls enough of a kind: "~ can't be blocked as long as defending
/// player controls three or more artifacts.", written from the twin about what you control.
fn defending_texts(value: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Some(condition) = value.get("Static").and_then(|body| body.get("condition")) else {
        return Vec::new();
    };
    let defending = json!({ "ControlledBy": "Defending" });
    let Some(counted) = condition
        .get("Compare")
        .and_then(|compare| compare.get("left"))
        .and_then(|left| left.get("Count"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    if !counted.contains(&defending) || value.to_string().contains("\"You\"") {
        return Vec::new();
    }
    let swapped: Vec<Value> = counted
        .iter()
        .map(|pred| {
            if pred == &defending {
                json!({ "ControlledBy": "You" })
            } else {
                pred.clone()
            }
        })
        .collect();
    let mut twin = value.clone();
    twin["Static"]["condition"]["Compare"]["left"]["Count"] = json!(swapped);
    let Some(twin) = ability_of(twin) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter(|text| text.matches("you control").count() == 1)
        .map(|text| text.replace("you control", "defending player controls"))
        .collect()
}

/// "As ~ enters, choose a card name." and "choose a nonland card name": written from the twin that chooses a creature
/// type.
fn card_name_texts(value: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Some(body) = value.get("Replacement").and_then(Value::as_object) else {
        return Vec::new();
    };
    let Some(by) = body.get("by") else {
        return Vec::new();
    };
    let named = match by.get("Choose").and_then(Value::as_str) {
        Some("CardName") => "a card name",
        Some("NonlandCardName") => "a nonland card name",
        _ => return Vec::new(),
    };
    let mut twin = value.clone();
    twin["Replacement"]["by"] = json!({ "Choose": "CreatureType" });
    let Some(twin) = ability_of(twin) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter(|text| text.contains("choose a creature type"))
        .map(|text| text.replace("choose a creature type", &format!("choose {named}")))
        .collect()
}

/// "Prevent all damage that would be dealt to ~ by creatures." and "Prevent all combat damage that would be dealt by
/// enchanted creature."
fn prevent_texts(value: &Value) -> Vec<String> {
    let Some(body) = value.get("Replacement").and_then(Value::as_object) else {
        return Vec::new();
    };
    if body.len() != 2 || body.get("by") != Some(&json!("Prevent")) {
        return Vec::new();
    }
    let Some(when) = body.get("when").and_then(|when| when.get("Damage")) else {
        return Vec::new();
    };
    let creatures = json!([{ "Type": "Creature" }, { "In": "Battlefield" }]);
    if when == &json!({ "to": "This", "source": creatures }) {
        return vec!["Prevent all damage that would be dealt to ~ by creatures".to_owned()];
    }
    let enchanted = json!([{ "Type": "Creature" }, { "Is": "Attached" }]);
    if when == &json!({ "source": enchanted }) {
        return ["enchanted", "equipped"]
            .iter()
            .map(|holder| format!("Prevent all damage that would be dealt by {holder} creature"))
            .collect();
    }
    if when == &json!({ "source": enchanted, "combat": true }) {
        return ["enchanted", "equipped"]
            .iter()
            .map(|holder| {
                format!("Prevent all combat damage that would be dealt by {holder} creature")
            })
            .collect();
    }
    Vec::new()
}

/// The steps of an ability, when it is a spell, a triggered or an activated ability.
fn steps(value: &Value) -> Option<&Vec<Value>> {
    ["Spell", "Triggered", "Activated"]
        .iter()
        .find_map(|key| value.get(key))?
        .get("effects")?
        .as_array()
}

/// The ability with its steps replaced.
fn with_steps(value: &Value, effects: Vec<Value>) -> Option<Ability> {
    let mut twin = value.clone();
    let key = ["Spell", "Triggered", "Activated"]
        .into_iter()
        .find(|key| value.get(key).is_some())?;
    twin[key]["effects"] = json!(effects);
    ability_of(twin)
}

/// "{2}: ~ becomes colorless until end of turn.": written from the twin that makes it black.
fn colorless_texts(value: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Some(effects) = steps(value) else {
        return Vec::new();
    };
    let colorless = json!({ "SetColors": [] });
    let at: Vec<usize> = effects
        .iter()
        .enumerate()
        .filter(|(_, effect)| {
            effect
                .get("Continuous")
                .and_then(|held| held.get("mods"))
                .and_then(Value::as_array)
                .is_some_and(|mods| mods.as_slice() == [colorless.clone()])
        })
        .map(|(at, _)| at)
        .collect();
    let ([at], false, 1) = (
        at.as_slice(),
        value.to_string().contains("Black"),
        effects.len(),
    ) else {
        return Vec::new();
    };
    let mut twin = effects.clone();
    twin[*at]["Continuous"]["mods"] = json!([{ "SetColors": ["Black"] }]);
    let Some(twin) = with_steps(value, twin) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter(|text| text.matches(" becomes black").count() == 1)
        .map(|text| text.replace(" becomes black", " becomes colorless"))
        .collect()
}

/// "{1}{G}: ~ can block an additional creature this turn.": written from the twin that can't be blocked this turn.
fn blocks_texts(value: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Some(effects) = steps(value) else {
        return Vec::new();
    };
    let mut found = None;
    for (at, effect) in effects.iter().enumerate() {
        let Some(held) = effect.get("Continuous") else {
            continue;
        };
        if held.get("duration") != Some(&json!("UntilEndOfTurn")) {
            continue;
        }
        if let Some([only]) = held.get("mods").and_then(Value::as_array).map(Vec::as_slice)
            && let Some(more) = only.get("BlocksAdditional").and_then(Value::as_u64)
        {
            if found.is_some() {
                return Vec::new();
            }
            found = Some((at, more));
        }
    }
    let Some((at, more)) = found else {
        return Vec::new();
    };
    if value.to_string().contains("CantBeBlocked") {
        return Vec::new();
    }
    let said = match more {
        1 => "can block an additional creature this turn".to_owned(),
        99 => "can block any number of creatures this turn".to_owned(),
        _ => match number(&json!(more)) {
            Some(more) => format!("can block {more} additional creatures this turn"),
            None => return Vec::new(),
        },
    };
    let mut twin = effects.clone();
    twin[at]["Continuous"]["mods"] = json!(["CantBeBlocked"]);
    let Some(twin) = with_steps(value, twin) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter(|text| text.matches("can't be blocked this turn").count() == 1)
        .map(|text| text.replace("can't be blocked this turn", &said))
        .collect()
}

/// "{T}: Target land becomes the basic land type of your choice until end of turn.": a land type chosen as the ability
/// resolves and given to the land, written from the twin that makes the land an Island.
fn land_type_choice_texts(value: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Some(effects) = steps(value) else {
        return Vec::new();
    };
    let Some(at) = effects.iter().position(|effect| {
        effect.get("ChooseValue").is_some_and(|held| {
            held.get("kind") == Some(&json!("BasicLandType"))
                && held.as_object().is_some_and(|map| map.len() == 2)
        })
    }) else {
        return Vec::new();
    };
    let Some(name) = effects[at]["ChooseValue"]["as"].as_str() else {
        return Vec::new();
    };
    let Some(next) = effects.get(at + 1) else {
        return Vec::new();
    };
    let chosen = json!([{ "SetLandTypesChosen": name }]);
    if next.get("Continuous").and_then(|held| held.get("mods")) != Some(&chosen)
        || value.to_string().contains("Island")
        || value.to_string().matches(name).count() != 2
    {
        return Vec::new();
    }
    let mut twin = effects.clone();
    twin[at + 1]["Continuous"]["mods"] = json!([{ "SetLandTypes": ["Island"] }]);
    twin.remove(at);
    let Some(twin) = with_steps(value, twin) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter(|text| text.matches(" becomes an Island").count() == 1)
        .map(|text| text.replace(" becomes an Island", " becomes the basic land type of your choice"))
        .collect()
}

/// The wordings of the condition of a cast in place of the mana cost: what a player controls (`control`), else both of
/// two conditions ("an opponent controls a Swamp and you control a Plains") or what the other writers say.
pub(super) fn wf27_if_texts(condition: &Condition, control: Vec<String>) -> Vec<String> {
    if !control.is_empty() {
        return control;
    }
    if let Ok(value) = serde_json::to_value(condition)
        && let Some(texts) = cast_condition_texts(&value)
    {
        return texts;
    }
    if let Condition::All(both) = condition
        && let [first, second] = both.as_slice()
    {
        let (firsts, seconds) = (wf27_if_texts(first, super::statics::control_texts(first)), {
            wf27_if_texts(second, super::statics::control_texts(second))
        });
        return firsts
            .iter()
            .flat_map(|first| seconds.iter().map(move |second| format!("{first} and {second}")))
            .collect();
    }
    condition_texts(condition)
}

/// The kind of a spell or permanent a condition counts, as the words before "spell": "white", "red instant or sorcery",
/// "green creature"; `None` for any other filter.
fn kind_words(preds: &[Value]) -> Option<String> {
    let mut color = None;
    let mut kinds = Vec::new();
    for pred in preds {
        if let Some(held) = pred.get("Color").and_then(Value::as_str) {
            color.is_none().then_some(())?;
            color = Some(held.to_lowercase());
        } else if let Some(held) = pred.get("Type").and_then(Value::as_str) {
            kinds.is_empty().then_some(())?;
            kinds.push(held.to_lowercase());
        } else if let Some(either) = pred.get("AnyOf").and_then(Value::as_array) {
            kinds.is_empty().then_some(())?;
            for kind in either {
                kinds.push(kind.get("Type")?.as_str()?.to_lowercase());
            }
        } else {
            return None;
        }
    }
    let kinds = kinds.join(" or ");
    Some(match (color, kinds.is_empty()) {
        (Some(color), true) => color,
        (Some(color), false) => format!("{color} {kinds}"),
        (None, _) => kinds,
    })
}

/// The wordings of a condition on the cast of the card itself, after "if": "it's not your turn", "you've cast another
/// spell this turn", "an opponent cast a red instant or sorcery spell this turn", "an opponent had an artifact enter the
/// battlefield under their control this turn", "an opponent searched their library this turn", "exactly one creature is
/// attacking", "there are thirteen or more creatures on the battlefield"; `None` for any other condition.
fn cast_condition_texts(condition: &Value) -> Option<Vec<String>> {
    if condition == "YourTurn" {
        return Some(vec!["it's your turn".to_owned()]);
    }
    if condition == &json!({ "Not": "YourTurn" }) {
        return Some(vec!["it's not your turn".to_owned()]);
    }
    if condition == &json!({ "Searched": { "who": "Opponents", "zone": "Library" } }) {
        return Some(vec!["an opponent searched their library this turn".to_owned()]);
    }
    // "not none" is "one or more".
    let compare = match condition.get("Not").and_then(|held| held.get("Compare")) {
        Some(held) if held.get("op")? == "Le" && held.get("right")? == &json!(0) => {
            json!({ "left": held.get("left")?, "op": "Ge", "right": 1 })
        }
        Some(_) => return None,
        None => condition.get("Compare")?.clone(),
    };
    let (left, op, right) = (compare.get("left")?, compare.get("op")?.as_str()?, compare.get("right")?);
    let right = right.as_u64()?;
    if let Some(tally) = left.get("Tally") {
        (op == "Ge" && right == 1 && tally.get("who")? == "EachPlayer").then_some(())?;
        let filter = tally.get("filter")?.as_array()?;
        let (whose, rest) = filter.split_last()?;
        let kind = kind_words(rest)?;
        let kind = if kind.is_empty() { kind } else { format!("{kind} ") };
        return match (tally.get("event")?.as_str()?, whose.get("ControlledBy")?.as_str()?) {
            ("SpellCast", "You") => Some(vec![
                format!("you've cast another {kind}spell this turn"),
                format!("you've cast {} {kind}spell this turn", article(&kind)),
            ]),
            ("SpellCast", "Opponents") => Some(vec![format!(
                "an opponent cast {} {kind}spell this turn",
                article(&kind)
            )]),
            ("Entered", "Opponents") if !kind.is_empty() => Some(vec![format!(
                "an opponent had {} {}enter the battlefield under their control this turn",
                article(&kind),
                kind
            )]),
            _ => None,
        };
    }
    let counted = left.get("Count")?;
    let many = number(&json!(right))?;
    if counted == &json!([{ "Type": "Creature" }, "Attacking"]) {
        return match (op, right) {
            ("Eq", 1) => Some(vec!["exactly one creature is attacking".to_owned()]),
            ("Ge", 2..) => Some(vec![format!("{many} or more creatures are attacking")]),
            _ => None,
        };
    }
    if counted == &json!([{ "Type": "Creature" }]) && op == "Ge" && right >= 2 {
        return Some(vec![format!(
            "there are {many} or more creatures on the battlefield"
        )]);
    }
    None
}

/// The moments a spell may be cast in that the w17c table leaves out, each with its wording after "only".
fn cast_moments(condition: &Value) -> Option<&'static str> {
    let steps = |names: &[&str]| -> Value {
        json!({ "Any": names.iter().map(|name| json!({ "Step": name })).collect::<Vec<_>>() })
    };
    if condition == &steps(&["BeginCombat", "DeclareAttackers"]) {
        return Some("during combat before blockers are declared");
    }
    if condition == &steps(&["DeclareBlockers", "FirstStrikeDamage", "CombatDamage", "EndCombat"]) {
        return Some("during combat after blockers are declared");
    }
    if condition
        == &steps(&[
            "Upkeep",
            "Draw",
            "Main1",
            "BeginCombat",
            "DeclareAttackers",
            "DeclareBlockers",
        ])
    {
        return Some("before the combat damage step");
    }
    if condition == "YourTurn" {
        return Some("during your turn");
    }
    if condition == &json!({ "Not": "YourTurn" }) {
        return Some("during an opponent's turn");
    }
    if condition == &json!({ "All": [{ "Step": "Upkeep" }, { "Not": "YourTurn" }] }) {
        return Some("during an opponent's upkeep");
    }
    None
}

/// "Cast this spell only if you've cast another spell this turn.", "Cast this spell only during combat before blockers
/// are declared.", "This spell costs {2} less to cast if you've cast another spell this turn.", and the free cast of a
/// spell with no cost said as paying {0}.
fn cast_texts(value: &Value) -> Vec<String> {
    if let Some(condition) = value.get("CastOnly").and_then(|body| body.get("condition")) {
        if let Some(moment) = cast_moments(condition) {
            return vec![format!("Cast ~ only {moment}")];
        }
        return cast_condition_texts(condition)
            .unwrap_or_default()
            .into_iter()
            .map(|held| format!("Cast ~ only if {held}"))
            .collect();
    }
    if let Some(body) = value.get("CostModifier").and_then(Value::as_object) {
        let plain = body.len() == 4
            && body.get("applies_to") == Some(&json!(["This"]))
            && body.get("change") == Some(&json!("Reduce"));
        let (Some(mana), Some(condition)) = (
            body.get("mana").and_then(Value::as_str),
            body.get("condition"),
        ) else {
            return Vec::new();
        };
        if !plain {
            return Vec::new();
        }
        return cast_condition_texts(condition)
            .unwrap_or_default()
            .into_iter()
            .map(|held| format!("~ costs {mana} less to cast if {held}"))
            .collect();
    }
    if let Some(body) = value.get("FreeCast").and_then(Value::as_object)
        && !body.contains_key("cost")
    {
        let Some(condition) = body.get("condition") else {
            return vec!["You may pay {0} rather than pay ~'s mana cost".to_owned()];
        };
        let Ok(held) = serde_json::from_value::<Condition>(condition.clone()) else {
            return Vec::new();
        };
        return wf27_if_texts(&held, super::statics::control_texts(&held))
            .into_iter()
            .map(|held| format!("If {held}, you may pay {{0}} rather than pay ~'s mana cost"))
            .collect();
    }
    Vec::new()
}
