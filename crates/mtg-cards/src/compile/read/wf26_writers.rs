//! wf26 writers for the shapes drafts use most: a player who reveals their hand and discards the card another player
//! chooses from it ("Target opponent reveals their hand. You choose a nonland card from it. That player discards that
//! card."), or every card of a kind ("Target player reveals their hand and discards all nonland cards."), kinship, a
//! creature type or color of your choice until end of turn, and the owner's choice of the top or bottom of their library.

use serde_json::Value;

use super::draws;
use crate::ability::Ability;
use crate::refs::{Ref, Selector};
use crate::value::Amount;

/// Adds the wordings of the wf26 twins: the revealed hand and its discard, kinship, a value of your choice, the top or
/// bottom of a library.
pub(super) fn wf26_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    wf26_revealed_wordings(ability, written, texts);
    wf26_kinship_wordings(ability, written, texts);
    wf26_chosen_wordings(ability, written, texts);
    wf26_top_or_bottom_wordings(ability, written, texts);
    wf26_boast_wordings(ability, written, texts);
    wf26_paid_wordings(ability, written, texts);
    let those: Vec<String> = texts.iter().filter_map(|text| those_named(text)).collect();
    for text in those {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// "Boast — {1}{W}: Untap ~." (CR 702.142): an ability activated only if the card attacked this turn and only once each
/// turn, written from its twin without the two limits.
fn wf26_boast_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(body) = value.get_mut("Activated").and_then(Value::as_object_mut) else {
        return;
    };
    let attacked =
        serde_json::json!({ "Matches": { "subject": "This", "selector": ["AttackedThisTurn"] } });
    if body.get("condition") != Some(&attacked)
        || body.get("limit").and_then(Value::as_str) != Some("OncePerTurn")
        || body.contains_key("timing")
    {
        return;
    }
    body.remove("condition");
    body.remove("limit");
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return;
    };
    let mut found = Vec::new();
    for text in written(&twin) {
        let wording = format!("Boast \u{2014} {text}");
        if !texts.contains(&wording) && !found.contains(&wording) && !text.contains("Activate ") {
            found.push(wording);
        }
    }
    texts.extend(found);
}

/// "Counter target spell unless its controller pays {2}. If they do, surveil 2.": the payment of another player and
/// what you do when they pay, written from the twin without it and from the steps alone.
fn wf26_paid_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(body) = value
        .as_object_mut()
        .and_then(|map| map.values_mut().next())
    else {
        return;
    };
    let Some(effects) = body.get_mut("effects").and_then(Value::as_array_mut) else {
        return;
    };
    // The payment is the last step, of a player other than you, and both of its outcomes do something.
    let Some(pay) = effects
        .last_mut()
        .and_then(|last| last.get_mut("Pay"))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    if pay.get("who").is_none_or(|who| who == "You") || !pay.contains_key("unpaid") {
        return;
    }
    let Some(paid) = pay.remove("paid") else {
        return;
    };
    let named = paid.to_string();
    if ["Target", "Bound", "Triggering", "ControllerOf"]
        .iter()
        .any(|word| named.contains(word))
    {
        return;
    }
    let (Ok(twin), Ok(steps)) = (
        serde_json::from_value::<Ability>(value),
        serde_json::from_value::<Ability>(serde_json::json!({ "Spell": { "effects": paid } })),
    ) else {
        return;
    };
    let after: Vec<String> = written(&steps)
        .iter()
        .filter(|text| !text.contains(". ") && text.ends_with('.'))
        .map(|text| lower_first(text))
        .collect();
    let mut found = Vec::new();
    for text in written(&twin) {
        if !text.ends_with('.') || !text.contains(" unless ") {
            continue;
        }
        for step in &after {
            for wording in [
                format!("{text} If they do, {step}"),
                format!("{text} If they do, you {step}"),
            ] {
                if !texts.contains(&wording) && !found.contains(&wording) {
                    found.push(wording);
                }
            }
        }
    }
    texts.extend(found);
}

/// The text with its first letter in lower case.
fn lower_first(text: &str) -> String {
    let mut letters = text.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(letters).collect()
    })
}

/// The plural nouns of objects "each of those ..." may name.
const PLURALS: [&str; 6] = [
    "creatures",
    "permanents",
    "artifacts",
    "lands",
    "planeswalkers",
    "enchantments",
];

/// "Tap up to two target creatures. Put a stun counter on each of them." as "... on each of those creatures": "them"
/// names the group named last, and "those" with its noun the same group.
fn those_named(text: &str) -> Option<String> {
    const THEM: &str = " each of them";
    let at = text.find(THEM)?;
    if text[at + THEM.len()..].contains(THEM) {
        return None;
    }
    let words: Vec<&str> = text[..at]
        .split(|c: char| !c.is_alphanumeric() && c != '~')
        .filter(|word| !word.is_empty())
        .collect();
    let last = words.iter().rev().find(|word| {
        PLURALS.contains(word)
            || matches!(
                **word,
                "them" | "they" | "cards" | "players" | "opponents" | "tokens" | "it" | "~"
            )
    })?;
    PLURALS
        .contains(last)
        .then(|| text.replacen(THEM, &format!(" each of those {last}"), 1))
}

/// "Target opponent reveals their hand. You choose a nonland card from it. That player discards that card.", "Target
/// player reveals their hand and discards all nonland cards.": the twin's plain discard said with the hand revealed.
fn wf26_revealed_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Some((twin, said, kind, chosen)) = revealed_twin(ability) else {
        return;
    };
    let mut found = Vec::new();
    for text in written(&twin) {
        let from = format!(" discards {said}");
        if text.matches(&from).count() != 1 {
            continue;
        }
        let tails: Vec<String> = if chosen {
            chosen_tails(&kind)
        } else {
            vec![format!(" reveals their hand and discards {kind}")]
        };
        for tail in tails {
            let wording = text.replacen(&from, &tail, 1);
            if !texts.contains(&wording) && !found.contains(&wording) {
                found.push(wording);
            }
        }
    }
    texts.extend(found);
}

/// The words of kinship between the head of the trigger and what a revealed card that shares a creature type does.
const KINSHIP: &str = "you may look at the top card of your library. If it shares a creature type with ~, you may \
                       reveal it. If you do,";

/// "At the beginning of your upkeep, you may look at the top card of your library. If it shares a creature type with
/// ~, you may reveal it. If you do, you gain 4 life." (kinship): the trigger written with the steps of the revealed card
/// alone, its kinship said before them.
fn wf26_kinship_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Some(twin) = kinship_twin(ability) else {
        return;
    };
    let mut found = Vec::new();
    for text in written(&twin) {
        let Some((head, rest)) = text.split_once(", ") else {
            continue;
        };
        if !head.starts_with("At the beginning of ") {
            continue;
        }
        let wording = format!("{head}, {KINSHIP} {rest}");
        if !texts.contains(&wording) && !found.contains(&wording) {
            found.push(wording);
        }
    }
    texts.extend(found);
}

/// The trigger with its kinship steps (the top card looked at, chosen when it shares a subtype with the card, revealed,
/// then the steps if one was) replaced by those steps; `None` for any other ability.
fn kinship_twin(ability: &Ability) -> Option<Ability> {
    let mut value = serde_json::to_value(ability).ok()?;
    let body = value.get_mut("Triggered")?.as_object_mut()?;
    if ["if", "limit", "targets", "optional"]
        .iter()
        .any(|key| body.contains_key(*key))
    {
        return None;
    }
    let effects = body.get_mut("effects")?;
    let [may] = effects.as_array()?.as_slice() else {
        return None;
    };
    let steps = may.pointer("/May/effects")?.as_array()?;
    let [look, choose, reveal, held] = steps.as_slice() else {
        return None;
    };
    let top = look.pointer("/LookAtTop/as")?.clone();
    let shown = choose.pointer("/Choose/as")?.clone();
    let then = held.pointer("/If/then")?.clone();
    let expected = serde_json::json!({ "May": { "effects": [
        { "LookAtTop": { "as": top } },
        { "Choose": {
            "from": { "Each": [{ "In": "Library" }, { "Is": { "Bound": top } }, { "SharesSubtype": "This" }] },
            "count": { "min": 0, "max": 1 },
            "as": shown,
        } },
        { "Reveal": { "what": { "Bound": shown } } },
        { "If": {
            "condition": { "CountOf": { "subject": { "Bound": shown }, "selector": [], "op": "Eq", "right": 1 } },
            "then": then,
        } },
    ] } });
    if *may != expected || reveal.is_null() {
        return None;
    }
    // The revealed card is named by none of the steps after it.
    if then.to_string().contains(&shown.to_string()) || then.to_string().contains(&top.to_string())
    {
        return None;
    }
    *effects = then;
    serde_json::from_value(value).ok()
}

/// The bonus the twins of [`wf26_chosen_wordings`] get in place of the value chosen, and its words.
const STAND_IN: &str = " gets +1/+1 until end of turn";

/// "{1}: ~ becomes the creature type of your choice until end of turn.", "{G}: ~ becomes the color of your choice until
/// end of turn.": a value you choose, then given to an object until end of turn.
fn wf26_chosen_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Some((twin, words)) = chosen_twin(ability) else {
        return;
    };
    let mut found = Vec::new();
    for text in written(&twin) {
        if text.matches(STAND_IN).count() != 1 {
            continue;
        }
        let wording = text.replacen(
            STAND_IN,
            &format!(" becomes the {words} of your choice until end of turn"),
            1,
        );
        if !texts.contains(&wording) && !found.contains(&wording) {
            found.push(wording);
        }
    }
    texts.extend(found);
}

/// The ability with a value you choose and its use until end of turn replaced by a stand-in bonus, and the words of the
/// value; `None` unless two steps in a row choose the value and give it, and nothing else names it.
fn chosen_twin(ability: &Ability) -> Option<(Ability, &'static str)> {
    let mut value = serde_json::to_value(ability).ok()?;
    let body = value.as_object_mut()?.values_mut().next()?;
    let effects = body.get_mut("effects")?.as_array_mut()?;
    let at = effects
        .iter()
        .position(|effect| effect.get("ChooseValue").is_some())?;
    let choose = effects.get(at)?.get("ChooseValue")?.as_object()?;
    if choose.contains_key("who") {
        return None;
    }
    let name = choose.get("as")?.clone();
    let (words, modification) = match choose.get("kind")?.as_str()? {
        "CreatureType" => ("creature type", "SetCreatureTypesChosen"),
        "Color" => ("color", "SetColorsChosen"),
        _ => return None,
    };
    let given = effects.get(at + 1)?.get("Continuous")?;
    let mods = serde_json::json!([{ modification: name }]);
    if given.get("mods")? != &mods
        || given.get("duration")? != "UntilEndOfTurn"
        || given.as_object()?.len() != 3
    {
        return None;
    }
    let affected = given.get("affected")?.clone();
    let named = name.to_string();
    if effects
        .iter()
        .enumerate()
        .any(|(i, effect)| i != at && i != at + 1 && effect.to_string().contains(&named))
    {
        return None;
    }
    effects[at + 1] = serde_json::json!({ "Continuous": {
        "affected": affected,
        "mods": [{ "ModifyPT": { "power": 1, "toughness": 1 } }],
        "duration": "UntilEndOfTurn",
    } });
    effects.remove(at);
    Some((serde_json::from_value(value).ok()?, words))
}

/// "Target creature's owner puts it on their choice of the top or bottom of their library.": the owner's choice of the
/// two places, written from the twin that puts the object on top.
fn wf26_top_or_bottom_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Some(twin) = top_or_bottom_twin(ability) else {
        return;
    };
    const ON_TOP: &str = " on top of its owner's library";
    let mut found = Vec::new();
    for text in written(&twin) {
        let Some(end) = text.find(ON_TOP) else {
            continue;
        };
        let before = &text[..end];
        let Some((lead, object)) = ["Put ", "put "]
            .iter()
            .filter_map(|verb| before.rfind(verb).map(|at| (at, verb.len())))
            .max_by_key(|(at, _)| *at)
            .map(|(at, len)| (&text[..at], &before[at + len..]))
        else {
            continue;
        };
        if !object.starts_with("target ") || object.contains(['.', ',']) {
            continue;
        }
        let subject = if lead.is_empty() || lead.ends_with(". ") {
            super::render::capitalize(object)
        } else {
            object.to_owned()
        };
        let wording = format!(
            "{lead}{subject}'s owner puts it on their choice of the top or bottom of their library{}",
            &text[end + ON_TOP.len()..]
        );
        if !texts.contains(&wording) && !found.contains(&wording) {
            found.push(wording);
        }
    }
    texts.extend(found);
}

/// The ability with the owner's choice of the top or the bottom of their library for an object replaced by the move to
/// the top; `None` unless the choice is the owner's and its two branches move the same object.
fn top_or_bottom_twin(ability: &Ability) -> Option<Ability> {
    let mut value = serde_json::to_value(ability).ok()?;
    let body = value.as_object_mut()?.values_mut().next()?;
    let effects = body.get_mut("effects")?.as_array_mut()?;
    let at = effects
        .iter()
        .position(|effect| effect.get("ChooseValue").is_some())?;
    let choose = effects.get(at)?.get("ChooseValue")?;
    let name = choose.get("as")?.clone();
    let what = choose.pointer("/who/OwnerOf")?.clone();
    let kind = serde_json::json!({ "Options": ["Top", "Bottom"] });
    let moved = |place: &str| serde_json::json!([{ "Move": { "what": what, "to": { "Library": { "place": place } } } }]);
    let expected = serde_json::json!({ "If": {
        "condition": { "ChosenWord": { "name": name, "word": "Bottom" } },
        "then": moved("Bottom"),
        "otherwise": moved("Top"),
    } });
    if choose.get("kind")? != &kind
        || choose.as_object()?.len() != 3
        || effects.get(at + 1)? != &expected
    {
        return None;
    }
    let named = name.to_string();
    if effects
        .iter()
        .enumerate()
        .any(|(i, effect)| i != at && i != at + 1 && effect.to_string().contains(&named))
    {
        return None;
    }
    effects[at + 1] = moved("Top").as_array()?.first()?.clone();
    effects.remove(at);
    serde_json::from_value(value).ok()
}

/// The printed tails of a hand revealed and a card chosen from it: "reveals their hand. You choose a nonland card from
/// it. That player discards that card", the quality of the card also after "from it", "They" for "That player".
fn chosen_tails(said: &str) -> Vec<String> {
    let mut kinds = vec![format!("{said} from it")];
    if let Some((kind, quality)) = said.split_once(" card with ") {
        kinds.push(format!("{kind} card from it with {quality}"));
    }
    let mut tails = Vec::new();
    for kind in kinds {
        for discards in ["That player discards", "They discard"] {
            tails.push(format!(
                " reveals their hand. You choose {kind}. {discards} that card"
            ));
        }
    }
    tails
}

/// The ability with a hand revealed and then discarded from written as the plain discard of one card, the words the
/// twin says that card with, the words of the cards discarded, and whether you choose the card. `None` unless one step
/// reveals the hand of the player who discards next.
fn revealed_twin(ability: &Ability) -> Option<(Ability, String, String, bool)> {
    let mut value = serde_json::to_value(ability).ok()?;
    let body = value.as_object_mut()?.values_mut().next()?;
    let effects = body.get_mut("effects")?.as_array_mut()?;
    let at = effects
        .windows(2)
        .position(|pair| pair_of(&pair[0], &pair[1]))?;
    let discard = effects
        .get_mut(at + 1)?
        .get_mut("Discard")?
        .as_object_mut()?;
    let by = discard.remove("by");
    let chosen = match by {
        None => false,
        Some(by) if by == "You" => true,
        Some(_) => return None,
    };
    if ["random", "optional", "bind"]
        .iter()
        .any(|key| discard.contains_key(*key))
    {
        return None;
    }
    let who: Ref = serde_json::from_value(discard.get("who")?.clone()).ok()?;
    let filter: Option<Selector> = match discard.get("filter") {
        Some(filter) => Some(serde_json::from_value(filter.clone()).ok()?),
        None => None,
    };
    // One chosen card, or every card of the kind in the hand; never a number the player picks from the revealed hand.
    let all = match discard.remove("count") {
        None => false,
        Some(count) => {
            every_card(&count, discard.get("who")?, discard.get("filter")).then_some(())?;
            true
        }
    };
    if chosen == all {
        return None;
    }
    let said = draws::discarded_filtered_text(&who, &Amount::Fixed(1), filter.as_ref())?;
    let kind = if all {
        // "a nonland card" as "all nonland cards".
        let one = said
            .strip_prefix("a ")
            .or_else(|| said.strip_prefix("an "))?;
        let many = match one.strip_prefix("card") {
            Some(rest) => format!("cards{rest}"),
            None => one.replacen(" card", " cards", 1),
        };
        format!("all {many}")
    } else {
        said.clone()
    };
    effects.remove(at);
    Some((serde_json::from_value(value).ok()?, said, kind, chosen))
}

/// Whether a discard's count is every card of its kind in the hand of the player who discards.
fn every_card(count: &Value, who: &Value, filter: Option<&Value>) -> bool {
    let Some(counted) = count.get("Count").and_then(Value::as_array) else {
        return false;
    };
    let mut wanted = vec![
        serde_json::json!({ "In": "Hand" }),
        serde_json::json!({ "OwnedBy": who }),
    ];
    if let Some(preds) = filter.and_then(Value::as_array) {
        wanted.extend(preds.iter().cloned());
    }
    counted.len() == wanted.len() && wanted.iter().all(|pred| counted.contains(pred))
}

/// Whether `reveal` shows the hand of the player who discards in `discard`.
fn pair_of(reveal: &Value, discard: &Value) -> bool {
    let (Some(shown), Some(who)) = (
        reveal
            .pointer("/Reveal/what/Each")
            .and_then(Value::as_array),
        discard.pointer("/Discard/who"),
    ) else {
        return false;
    };
    let owned = serde_json::json!({ "OwnedBy": who });
    let hand = serde_json::json!({ "In": "Hand" });
    shown.len() == 2 && shown.contains(&owned) && shown.contains(&hand)
}
