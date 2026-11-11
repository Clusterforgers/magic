//! wf27 writers for the shapes drafts use most: maximum hand sizes, the top card of each library revealed, a creature
//! that crews as though stronger, a life floor, landwalk ignored, lands made a basic land type, permanents that don't
//! untap, a card name chosen as a permanent enters, damage to or by a creature prevented, casting conditions and casts in
//! place of the mana cost, colorless and extra blocks until end of turn, the basic land type of your choice, a described
//! token that is a predefined one, amounts a kicker changes, damage counting the sacrificed permanent and a color chosen
//! for good.

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
    if word.starts_with(['A', 'E', 'I', 'O', 'U', 'a', 'e', 'i', 'o', 'u'])
        || word.starts_with("hour")
    {
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

/// Adds the wordings of the wf25, wf26 and wf27 twins.
pub(super) fn wf25_to_wf27_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    super::wf26_writers::wf25_wf26_wordings(ability, written, texts);
    wf27_wordings(ability, written, texts);
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
    if let Some(named) = tokens_named(&value) {
        found.extend(written(&named));
    }
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
    found.extend(sacrifice_and_counter_texts(&value, written));
    found.extend(kicked_texts(&value, written));
    found.extend(sacrificed_value_texts(&value, written));
    found.extend(chosen_color_texts(&value, written));
    found.extend(five_colors_texts(&value, written));
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
    if body.contains_key("condition") {
        return conditional_rule_texts(body);
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

/// "As long as you have 7 or more life, damage that would reduce your life total to less than 7 reduces it to 7
/// instead.": the rule while the condition holds.
fn conditional_rule_texts(body: &serde_json::Map<String, Value>) -> Vec<String> {
    let Some(condition) = body.get("condition") else {
        return Vec::new();
    };
    let Ok(held) = serde_json::from_value::<Condition>(condition.clone()) else {
        return Vec::new();
    };
    if body.len() != 2 {
        return Vec::new();
    }
    let plain = json!({ "Rule": { "rule": body.get("rule") } });
    if !rule_texts_conditional(&plain) {
        return Vec::new();
    }
    let mut texts = Vec::new();
    for rule in &rule_texts(&plain) {
        let rule = lowered(rule);
        for held in condition_texts(&held) {
            texts.push(format!("As long as {held}, {rule}"));
        }
        for held in super::statics::control_texts(&held) {
            texts.push(format!("If {held}, {rule}"));
        }
    }
    texts
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
        return conditional_untap_texts(affected, condition);
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

/// "~ doesn't untap during your untap step if an opponent controls two or more creatures.": the card or an enchanted
/// creature that doesn't untap while a condition holds.
fn conditional_untap_texts(affected: &Value, condition: &Value) -> Vec<String> {
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
    texts
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
    if let Some(text) = super::triggers::condition_text(condition)
        && !texts.contains(&text)
    {
        texts.push(text);
    }
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
fn with_steps(value: &Value, effects: &[Value]) -> Option<Ability> {
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
    let Some(twin) = with_steps(value, &twin) else {
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
        if let Some([only]) = held
            .get("mods")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
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
    let Some(twin) = with_steps(value, &twin) else {
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
    let Some(twin) = with_steps(value, &twin) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter(|text| text.matches(" becomes an Island").count() == 1)
        .map(|text| {
            text.replace(
                " becomes an Island",
                " becomes the basic land type of your choice",
            )
        })
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
        let (firsts, seconds) = (
            wf27_if_texts(first, super::statics::control_texts(first)),
            { wf27_if_texts(second, super::statics::control_texts(second)) },
        );
        return firsts
            .iter()
            .flat_map(|first| {
                seconds
                    .iter()
                    .map(move |second| format!("{first} and {second}"))
            })
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
        } else {
            let either = pred.get("AnyOf").and_then(Value::as_array)?;
            kinds.is_empty().then_some(())?;
            for kind in either {
                kinds.push(kind.get("Type")?.as_str()?.to_lowercase());
            }
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
    let condition = &unwrapped(condition);
    let left = |what: &str| json!({ "Compare": { "left": { "Tallied": { "who": "EachPlayer", "what": what } }, "op": "Ge", "right": 1 } });
    if condition == &json!({ "Any": [left("NonlandPermanentsLeft"), left("SpellsWarped")] }) {
        return Some(vec![
            "a nonland permanent left the battlefield this turn or a spell was warped this turn"
                .to_owned(),
        ]);
    }
    if condition == "YourTurn" {
        return Some(vec!["it's your turn".to_owned()]);
    }
    if condition == &json!({ "Not": "YourTurn" }) {
        return Some(vec!["it's not your turn".to_owned()]);
    }
    if condition == &json!({ "Searched": { "who": "Opponents", "zone": "Library" } }) {
        return Some(vec![
            "an opponent searched their library this turn".to_owned(),
        ]);
    }
    // "not none" is "one or more".
    let compare = match condition.get("Not").and_then(|held| held.get("Compare")) {
        Some(held) if held.get("op")? == "Le" && held.get("right")? == &json!(0) => {
            json!({ "left": held.get("left")?, "op": "Ge", "right": 1 })
        }
        Some(_) => return None,
        None => condition.get("Compare")?.clone(),
    };
    let (left, op, right) = (
        compare.get("left")?,
        compare.get("op")?.as_str()?,
        compare.get("right")?,
    );
    let right = right.as_u64()?;
    if let Some(tally) = left.get("Tally") {
        (op == "Ge" && right == 1 && tally.get("who")? == "EachPlayer").then_some(())?;
        let filter = tally.get("filter")?.as_array()?;
        let (whose, rest) = filter.split_last()?;
        let kind = kind_words(rest)?;
        let kind = if kind.is_empty() {
            kind
        } else {
            format!("{kind} ")
        };
        return match (
            tally.get("event")?.as_str()?,
            whose.get("ControlledBy")?.as_str()?,
        ) {
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
    if condition
        == &steps(&[
            "DeclareBlockers",
            "FirstStrikeDamage",
            "CombatDamage",
            "EndCombat",
        ])
    {
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
        let held = cast_condition_texts(condition).unwrap_or_else(|| {
            serde_json::from_value::<Condition>(unwrapped(condition))
                .map(|held| condition_texts(&held))
                .unwrap_or_default()
        });
        return held
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

/// "When a player casts a spell, sacrifice ~ and counter that spell.": the two steps of one sentence joined by "and"
/// read the same in either order, so the steps in the other order are written from their twin.
fn sacrifice_and_counter_texts(value: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Some(effects) = steps(value) else {
        return Vec::new();
    };
    let counter = json!({ "Counter": { "what": "Triggering" } });
    let sacrifice = json!({ "Sacrifice": { "filter": ["This"] } });
    if effects.as_slice() != [counter.clone(), sacrifice.clone()] {
        return Vec::new();
    }
    let Some(twin) = with_steps(value, &[sacrifice, counter]) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter(|text| text.contains(", sacrifice ~ and counter that spell"))
        .collect()
}

/// The ability with each described token that is a predefined one ("a Lander token") named by its id; `None` when it
/// describes none.
fn tokens_named(value: &Value) -> Option<Ability> {
    fn walk(value: &mut Value, named: &[(Value, &str)], found: &mut bool) {
        match value {
            Value::Object(map) => {
                if let Some(token) = map.get_mut("token")
                    && let Some(spec) = token.get("Custom")
                    && let Some((_, id)) = named.iter().find(|(known, _)| known == spec)
                {
                    *token = json!({ "Named": id });
                    *found = true;
                }
                for inner in map.values_mut() {
                    walk(inner, named, found);
                }
            }
            Value::Array(items) => {
                for inner in items {
                    walk(inner, named, found);
                }
            }
            _ => {}
        }
    }
    if !value.to_string().contains("\"Custom\"") {
        return None;
    }
    let named: Vec<(Value, &str)> = super::create::NAMED
        .iter()
        .filter_map(|(_, id)| {
            let spec = crate::builtin::token_spec(id)?;
            Some((serde_json::to_value(spec).ok()?, *id))
        })
        .collect();
    let mut twin = value.clone();
    let mut found = false;
    walk(&mut twin, &named, &mut found);
    found.then(|| ability_of(twin)).flatten()
}

/// The value with each amount that depends on the kicker replaced by its amount when kicked (`then`) or not.
fn kicked_twin(value: &Value, kicked: bool) -> Value {
    match value {
        Value::Object(map) => {
            if let Some(when) = map.get("When")
                && when.get("condition") == Some(&json!("Kicked"))
                && map.len() == 1
            {
                let key = if kicked { "then" } else { "otherwise" };
                return when.get(key).cloned().unwrap_or(Value::Null);
            }
            let folded: serde_json::Map<String, Value> = map
                .iter()
                .map(|(key, inner)| (key.clone(), kicked_twin(inner, kicked)))
                .collect();
            // "-2/-2" is the product of the amount and -1 once the amount is a number.
            if let Some(times) = folded.get("Times")
                && folded.len() == 1
                && let (Some(by), Some(of)) = (
                    times.get("by").and_then(Value::as_i64),
                    times.get("of").and_then(Value::as_i64),
                )
                && map
                    .get("Times")
                    .is_some_and(|held| held.to_string().contains("Kicked"))
            {
                return json!(by * of);
            }
            Value::Object(folded)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|inner| kicked_twin(inner, kicked))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// The step said again for the kicked spell, naming what the first sentence named: "that creature gets +5/+5 until end
/// of turn", "that player discards three cards", "it deals 5 damage to that creature", "those creatures get +2/+1 until
/// end of turn".
fn kicked_again(text: &str, step: &Value) -> Vec<String> {
    let text = text.trim_end_matches('.');
    let targets = text.matches("target ").count() + text.matches("Target ").count();
    if text.contains(". ") || targets > 1 {
        return Vec::new();
    }
    let kind = step
        .as_object()
        .and_then(|map| map.keys().next())
        .map_or("", String::as_str);
    if kind == "DealDamage"
        && let Some(rest) = text.strip_prefix("~ ")
    {
        // "it deals 5 damage to that creature", or "it deals 5 damage" with the recipient left as the first sentence said.
        return match rest.split_once(" to ") {
            Some((deals, to)) if to.starts_with("target ") || to == "any target" => {
                let to = to.strip_prefix("target ").map_or_else(
                    || "that permanent or player".to_owned(),
                    |noun| format!("that {noun}"),
                );
                vec![format!("it {deals} to {to}"), format!("it {deals}")]
            }
            _ => vec![format!("it {rest}")],
        };
    }
    if kind == "Shield"
        && let Some((head, tail)) = text.split_once(" to any target")
        && text.starts_with("Prevent the next ")
    {
        return vec![format!(
            "prevent{} to that permanent or player{tail}",
            &head["Prevent".len()..]
        )];
    }
    if let Some(rest) = text.strip_prefix("Target ") {
        return vec![format!("that {rest}")];
    }
    if text.starts_with("Creatures ")
        && let Some((_, rest)) = text.split_once(" get ")
    {
        return vec![format!("those creatures get {rest}")];
    }
    Vec::new()
}

/// Whether the step does nothing unkicked: a bonus of +0/+0.
fn unkicked_has_nothing(step: &Value) -> bool {
    kicked_twin(step, false)
        .to_string()
        .contains("\"ModifyPT\":{\"power\":0,\"toughness\":0}")
}

/// "Target creature gets +2/+2 until end of turn. If this spell was kicked, that creature gets +5/+5 until end of turn
/// instead.": a spell whose one step has amounts that depend on the kicker, written from its twins unkicked and kicked.
fn kicked_texts(value: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Some(body) = value.get("Spell") else {
        return Vec::new();
    };
    let Some(effects) = steps(value) else {
        return Vec::new();
    };
    let kicked: Vec<usize> = effects
        .iter()
        .enumerate()
        .filter(|(_, effect)| effect.to_string().contains("\"condition\":\"Kicked\""))
        .map(|(at, _)| at)
        .collect();
    let [at] = kicked.as_slice() else {
        return Vec::new();
    };
    if body.to_string().matches("\"Kicked\"").count()
        != effects[*at].to_string().matches("\"Kicked\"").count()
    {
        return Vec::new();
    }
    let unkicked = kicked_twin(value, false);
    if &unkicked == value {
        return Vec::new();
    }
    let (Some(plain), Some(step)) = (
        ability_of(unkicked),
        with_steps(
            &kicked_twin(value, true),
            &[kicked_twin(&effects[*at], true)],
        ),
    ) else {
        return Vec::new();
    };
    // A step the kicker only adds to ("gets +0/+0", then "+1/+1" when kicked) is no step said again.
    if unkicked_has_nothing(&effects[*at]) {
        return Vec::new();
    }
    let again: Vec<String> = written(&step)
        .iter()
        .flat_map(|text| kicked_again(text, &effects[*at]))
        .collect();
    let mut texts = Vec::new();
    for first in written(&plain) {
        let first = first.trim_end_matches('.');
        for again in &again {
            for head in ["If this spell was kicked", "If it was kicked"] {
                texts.push(format!("{first}. {head}, {again} instead"));
            }
        }
    }
    texts
}

/// A condition held as a number ("one when C, else none" is at least one) as the condition itself.
fn unwrapped(condition: &Value) -> Value {
    let held = condition.get("Compare").and_then(|compare| {
        (compare.get("op")? == "Ge" && compare.get("right")? == &json!(1)).then_some(())?;
        let when = compare.get("left")?.get("When")?;
        (when.get("then")? == &json!(1) && when.get("otherwise")? == &json!(0)).then_some(())?;
        when.get("condition").cloned()
    });
    held.unwrap_or_else(|| condition.clone())
}

/// "Sacrifice another creature or artifact: ~ deals damage equal to 2 plus the sacrificed permanent's mana value to any
/// target.": the damage counts the permanent sacrificed to pay the cost, written from the twin that deals a fixed amount.
fn sacrificed_value_texts(value: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Some(body) = value.get("Activated") else {
        return Vec::new();
    };
    let sacrifices = body
        .get("cost")
        .and_then(Value::as_array)
        .is_some_and(|cost| {
            cost.iter()
                .filter(|item| item.get("Sacrifice").is_some())
                .count()
                == 1
        });
    let Some(effects) = steps(value) else {
        return Vec::new();
    };
    let [only] = effects.as_slice() else {
        return Vec::new();
    };
    let Some(amount) = only.get("DealDamage").and_then(|deal| deal.get("amount")) else {
        return Vec::new();
    };
    let paid = json!({ "Stat": { "of": "Paid", "stat": "ManaValue" } });
    let more = match amount
        .get("Sum")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    {
        Some([held, more]) if held == &paid => more.as_u64(),
        _ => None,
    };
    let (true, Some(more)) = (sacrifices, more) else {
        return Vec::new();
    };
    let mut twin = only.clone();
    twin["DealDamage"]["amount"] = json!(999);
    let Some(twin) = with_steps(value, &[twin]) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter(|text| text.matches(" deals 999 damage").count() == 1)
        .map(|text| {
            text.replace(
                " deals 999 damage",
                &format!(
                    " deals damage equal to {more} plus the sacrificed permanent's mana value"
                ),
            )
        })
        .collect()
}

/// "When ~ enters, draw a card, then choose a color. ~ becomes the chosen color.": a color chosen as the last steps and
/// given to the card for good, written from the twin without them.
fn chosen_color_texts(value: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Some(effects) = steps(value) else {
        return Vec::new();
    };
    let [rest @ .., choose, gives] = effects.as_slice() else {
        return Vec::new();
    };
    let Some(name) = choose
        .get("ChooseValue")
        .filter(|held| {
            held.get("kind") == Some(&json!("Color"))
                && held.as_object().is_some_and(|map| map.len() == 2)
        })
        .and_then(|held| held.get("as"))
    else {
        return Vec::new();
    };
    let forever = json!({ "Continuous": { "affected": "This", "mods": [{ "SetColorsChosen": name }], "duration": "Forever" } });
    if gives != &forever
        || rest.is_empty()
        || value.to_string().matches(&name.to_string()).count() != 2
    {
        return Vec::new();
    }
    let Some(twin) = with_steps(value, rest) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .map(|text| {
            format!(
                "{}, then choose a color. ~ becomes the chosen color",
                text.trim_end_matches('.')
            )
        })
        .collect()
}

/// "{3}, {T}: Draw a card. Activate only if there are five colors among permanents you control.", written from the twin
/// without the condition.
fn five_colors_texts(value: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Some(body) = value.get("Activated").and_then(Value::as_object) else {
        return Vec::new();
    };
    let five = json!({ "Compare": { "left": { "Distinct": { "of": [{ "ControlledBy": "You" }], "stat": "Colors" } }, "op": "Eq", "right": 5 } });
    if body.get("condition") != Some(&five) {
        return Vec::new();
    }
    let mut twin = value.clone();
    if let Some(map) = twin["Activated"].as_object_mut() {
        map.remove("condition");
    }
    let Some(twin) = ability_of(twin) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter(|text| !text.contains("Activate "))
        .map(|text| {
            format!(
                "{} Activate only if there are five colors among permanents you control",
                text.trim_end_matches('.').to_owned() + "."
            )
        })
        .collect()
}

/// Other words for what the writers say: "that spell's mana value" for the mana value of the spell that was cast,
/// "with lesser power" for "with power less than ~'s", "for as long as you control ~", and the Ring's temptation as a
/// sentence of its own.
pub(super) fn wf27_last_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let Some(value) = value_of(ability) else {
        return;
    };
    for text in leftover_wordings(&value, texts) {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

fn leftover_wordings(value: &Value, texts: &[String]) -> Vec<String> {
    let cast = value
        .get("Triggered")
        .and_then(|body| body.get("when"))
        .is_some_and(|when| when.get("SpellCast").is_some());
    let lesser = holds(
        value,
        &json!({ "Compare": { "stat": "Power", "op": "Lt", "to": { "Stat": { "of": "This", "stat": "Power" } } } }),
    );
    let mut found = Vec::new();
    for text in texts {
        let mut said = text.clone();
        if cast {
            said = said.replace(
                "with mana value equal to that creature's mana value",
                "with that spell's mana value",
            );
        }
        if lesser {
            said = said
                .replace("with power less than ~'s power", "with lesser power")
                .replace("with power less than ~'s", "with lesser power")
                .replace(
                    "for as long as ~ you control",
                    "for as long as you control ~",
                )
                .replace(", then the Ring tempts you", ". Then the Ring tempts you");
        }
        if &said != text {
            found.push(said);
        }
    }
    found
}

/// Whether `needle` is `value` or a part of it.
fn holds(value: &Value, needle: &Value) -> bool {
    value == needle
        || match value {
            Value::Object(map) => map.values().any(|inner| holds(inner, needle)),
            Value::Array(items) => items.iter().any(|inner| holds(inner, needle)),
            _ => false,
        }
}
