//! Whole-ability wordings of the r30-0c card pack, each written from a twin another writer words with one part changed:
//! "Put up to X +1/+0 counters on this creature. This ability can't cause the total number of +1/+0 counters on this
//! creature to be greater than four." is a chosen number bounded by X and by the room left under the cap, then that
//! many counters.

use serde_json::{Value, json};

use super::amounts::count_word;
use crate::ability::Ability;

/// Adds the r30-0c wordings of the ability to `texts`; `written` words a twin.
pub(super) fn r300c_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let mut found = capped_counters(ability, written);
    found.extend(any_number_removed(ability, written));
    found.extend(counter_instead(ability, written));
    found.extend(pump_and_choice(ability, written));
    found.extend(snow_kept(ability, written));
    found.extend(returned_from_graveyard(ability, written));
    found.extend(mana_artifact_or_basic(ability, written));
    found.extend(milled_kept(ability, written));
    found.extend(life_rather(ability, written));
    found.extend(proliferate_per_type(ability, written));
    found.extend(exiled_at_random(ability, written));
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// The name the capped number is chosen under.
const CAPPED: &str = "n";

/// "Put up to X <kind> counters on this creature. This ability can't cause the total number of <kind> counters on this
/// creature to be greater than <cap>.": `ChooseValue` of a number from 0 to the lesser of X and the cap less the counters
/// there, then `PutCounters` of that many on the source; the twin puts X counters.
fn capped_counters(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let Some(effects) = value.pointer("/Activated/effects").cloned() else {
        return Vec::new();
    };
    let Some((counter, cap)) = capped_shape(&effects) else {
        return Vec::new();
    };
    let Some(cap_word) = u32::try_from(cap).ok().and_then(count_word) else {
        return Vec::new();
    };
    let Some(slot) = value.pointer_mut("/Activated/effects") else {
        return Vec::new();
    };
    *slot = json!([{"PutCounters": {"on": "This", "counter": counter.clone(), "count": "X"}}]);
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    let Ok(kind) = serde_json::from_value::<crate::value::CounterKind>(counter) else {
        return Vec::new();
    };
    let kind = kind.name();
    let mut texts = Vec::new();
    for text in written(&twin) {
        for holder in ["this creature", "~"] {
            let plain = format!("Put X {kind} counters on {holder}.");
            if text.contains(&plain) {
                let capped = format!(
                    "Put up to X {kind} counters on {holder}. This ability can't cause the total number of {kind} \
                     counters on {holder} to be greater than {cap_word}."
                );
                texts.push(text.replacen(&plain, &capped, 1));
            }
        }
    }
    texts
}

/// The counter kind and the cap of the two steps of a capped put, when the effects are exactly that shape.
fn capped_shape(effects: &Value) -> Option<(Value, i64)> {
    let [choose, put] = effects.as_array()?.as_slice() else {
        return None;
    };
    let put = put.get("PutCounters")?;
    if put.get("on") != Some(&json!("This"))
        || put.get("count") != Some(&json!({"ChosenAs": CAPPED}))
    {
        return None;
    }
    let counter = put.get("counter")?.clone();
    let choose = choose.get("ChooseValue")?;
    if choose.get("as") != Some(&json!(CAPPED)) || choose.get("who").is_some() {
        return None;
    }
    let bounds = choose.pointer("/kind/Number")?.as_array()?;
    let [low, high] = bounds.as_slice() else {
        return None;
    };
    if *low != json!(0) {
        return None;
    }
    let cap = high.pointer("/When/otherwise/Difference/0")?.as_i64()?;
    let room = json!({"Difference": [cap, {"Counters": {"on": "This", "counter": counter}}]});
    let wanted = json!({"When": {
        "condition": {"Compare": {"left": "X", "op": "Le", "right": room}},
        "then": "X",
        "otherwise": room,
    }});
    (*high == wanted && cap > 0).then_some((counter, cap))
}

/// "Remove any number of counters from <it>": `RemoveCounters` of any kinds, up to all the counters there; the twin
/// removes up to three.
fn any_number_removed(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let Some(path) = ["/Activated/effects", "/Spell/effects", "/Triggered/effects"]
        .into_iter()
        .find(|path| value.pointer(path).is_some())
    else {
        return Vec::new();
    };
    let Some(effects) = value.pointer_mut(path).and_then(Value::as_array_mut) else {
        return Vec::new();
    };
    let mut changed = false;
    for effect in effects.iter_mut() {
        let Some(remove) = effect
            .get_mut("RemoveCounters")
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        let all = remove.get("from").map(|from| json!({"AnyCounters": from}));
        if remove.get("counter").is_none()
            && remove.get("up_to") == Some(&json!(true))
            && all.is_some()
            && remove.get("count") == all.as_ref()
        {
            remove.insert("count".to_owned(), json!(3));
            changed = true;
        }
    }
    if !changed {
        return Vec::new();
    }
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter(|text| text.contains("emove up to three counters from "))
        .map(|text| {
            text.replace(
                "emove up to three counters from ",
                "emove any number of counters from ",
            )
        })
        .collect()
}

/// The conditions on the mana an activation spent, with their words: "exactly three colors of mana were spent" and "mana
/// from a Treasure was spent".
fn spent_conditions() -> Vec<(Value, &'static str)> {
    let canonical = |value: Value| {
        serde_json::from_value::<crate::value::Condition>(value)
            .ok()
            .and_then(|condition| serde_json::to_value(condition).ok())
    };
    [
        (
            json!({"Compare": {"left": "ColorsSpent", "op": "Eq", "right": 3}}),
            "exactly three colors of mana were spent",
        ),
        (
            json!({"Compare": {"left": {"ManaSpent": {"of": null, "from": {"subtypes": ["Treasure"]}}}, "op": "Ge", "right": 1}}),
            "mana from a Treasure was spent",
        ),
    ]
    .into_iter()
    .filter_map(|(value, words)| canonical(value).map(|value| (value, words)))
    .collect()
}

/// "This creature gets +1/+1 until end of turn. If <mana> was spent to activate this ability, put a +1/+1 counter on it
/// instead.": an `If` on the activation's mana whose `then` puts the counter and whose `otherwise` is the pump; the twin
/// is the pump alone.
fn counter_instead(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let Some(Value::Array(effects)) = value.pointer("/Activated/effects").cloned() else {
        return Vec::new();
    };
    let [step] = effects.as_slice() else {
        return Vec::new();
    };
    let Some(choice) = step.get("If") else {
        return Vec::new();
    };
    let counter = json!([{"PutCounters": {"on": "This", "counter": "+1/+1"}}]);
    let pump = json!([{"Continuous": {
        "affected": "This",
        "mods": [{"ModifyPT": {"power": 1, "toughness": 1}}],
        "duration": "UntilEndOfTurn",
    }}]);
    if choice.get("then") != Some(&counter) || choice.get("otherwise") != Some(&pump) {
        return Vec::new();
    }
    let Some(words) = spent_conditions()
        .into_iter()
        .find(|(condition, _)| choice.get("condition") == Some(condition))
        .map(|(_, words)| words)
    else {
        return Vec::new();
    };
    let Some(slot) = value.pointer_mut("/Activated/effects") else {
        return Vec::new();
    };
    *slot = pump;
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    let mut texts = Vec::new();
    for text in written(&twin) {
        let Some(head) = text.strip_suffix('.') else {
            continue;
        };
        for holder in ["it", "this creature", "~"] {
            texts.push(format!(
                "{head}. If {words} to activate this ability, put a +1/+1 counter on {holder} instead."
            ));
        }
    }
    texts
}

/// The keyword of a chain of choices ("your choice of A, B, or C"): each `Either` gives its first branch one keyword and
/// its second the rest of the chain, every branch a `Continuous` of one keyword on the same objects for the same time.
fn chained_keywords(effect: &Value, affected: &Value, duration: &Value) -> Option<Vec<Value>> {
    let grant = |branch: &Value| -> Option<Value> {
        let [step] = branch.as_array()?.as_slice() else {
            return None;
        };
        let body = step.get("Continuous")?;
        if body.get("affected") != Some(affected) || body.get("duration") != Some(duration) {
            return None;
        }
        let [modification] = body.get("mods")?.as_array()?.as_slice() else {
            return None;
        };
        modification.get("AddKeyword").cloned()
    };
    let either = effect.get("Either")?;
    let mut keywords = vec![grant(either.get("first")?)?];
    let second = either.get("second")?;
    if let Some(last) = grant(second) {
        keywords.push(last);
    } else {
        let [inner] = second.as_array()?.as_slice() else {
            return None;
        };
        keywords.extend(chained_keywords(inner, affected, duration)?);
    }
    Some(keywords)
}

/// "This creature gets -1/-1 and gains your choice of banding, flying, first strike, or trample until end of turn.": a
/// pump and a chain of four or more keyword choices on the source; the twins are the pump with each keyword.
fn pump_and_choice(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let Some(Value::Array(effects)) = value.pointer("/Activated/effects") else {
        return Vec::new();
    };
    let [pump, choice] = effects.as_slice() else {
        return Vec::new();
    };
    let Some(body) = pump.get("Continuous") else {
        return Vec::new();
    };
    let (Some(affected), Some(duration), Some(Value::Array(mods))) =
        (body.get("affected"), body.get("duration"), body.get("mods"))
    else {
        return Vec::new();
    };
    if affected != &json!("This") || duration != &json!("UntilEndOfTurn") {
        return Vec::new();
    }
    let [modify] = mods.as_slice() else {
        return Vec::new();
    };
    if modify.get("ModifyPT").is_none() {
        return Vec::new();
    }
    let Some(keywords) = chained_keywords(choice, affected, duration) else {
        return Vec::new();
    };
    if keywords.len() < 4 {
        return Vec::new();
    }
    // Each twin is the pump with one keyword: "gets -1/-1 and gains flying until end of turn".
    let mut heads = Vec::new();
    let mut words = Vec::new();
    for keyword in &keywords {
        let mut twin = value.clone();
        let Some(slot) = twin.pointer_mut("/Activated/effects") else {
            return Vec::new();
        };
        *slot = json!([{"Continuous": {
            "affected": affected,
            "mods": [modify, {"AddKeyword": keyword}],
            "duration": duration,
        }}]);
        let Ok(twin) = serde_json::from_value::<Ability>(twin) else {
            return Vec::new();
        };
        let Some((head, word)) = written(&twin).iter().find_map(|text| {
            let (head, rest) = text.split_once(" and gains ")?;
            let word = rest.strip_suffix(" until end of turn.")?;
            Some((head.to_owned(), word.to_owned()))
        }) else {
            return Vec::new();
        };
        heads.push(head);
        words.push(word);
    }
    let Some(head) = heads.first() else {
        return Vec::new();
    };
    if heads.iter().any(|other| other != head) {
        return Vec::new();
    }
    let Some((last, rest)) = words.split_last() else {
        return Vec::new();
    };
    vec![format!(
        "{head} and gains your choice of {}, or {last} until end of turn.",
        rest.join(", ")
    )]
}

/// "Look at the top card of your library. If that card is snow, you may reveal it and put it into your hand.": the look,
/// a choice of up to one among the top card by the snow supertype alone, its reveal and its move to the hand; the twin
/// draws a card.
fn snow_kept(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let steps = json!([
        {"LookAtTop": {"as": "top"}},
        {"Choose": {
            "from": {"Each": [{"Is": {"Bound": "top"}}, {"In": "Library"}, {"Supertype": "Snow"}]},
            "count": {"min": 0, "max": 1},
            "as": "kept",
        }},
        {"Reveal": {"what": {"Bound": "kept"}}},
        {"Move": {"what": {"Bound": "kept"}, "to": "Hand"}},
    ]);
    let Some(slot) = value.pointer_mut("/Activated/effects") else {
        return Vec::new();
    };
    if *slot != steps {
        return Vec::new();
    }
    *slot = json!([{"Draw": {}}]);
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter(|text| text.ends_with(": Draw a card."))
        .map(|text| {
            text.replace(
                ": Draw a card.",
                ": Look at the top card of your library. If that card is snow, you may reveal it and put it into your \
                 hand.",
            )
        })
        .collect()
}

/// "Mill five cards. Put all creature cards with activated abilities that aren't mana abilities from among the milled
/// cards into your hand.": a mill bound to a name, then every milled creature card with such an ability moved to the
/// hand; the twin mills alone.
fn milled_kept(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let Some(Value::Array(effects)) = value.pointer_mut("/Activated/effects") else {
        return Vec::new();
    };
    let kept = json!({"Move": {
        "what": {"Each": [
            {"Is": {"Bound": "milled"}},
            {"In": "Graveyard"},
            {"Type": "Creature"},
            {"HasAbility": "NonManaActivated"},
        ]},
        "to": "Hand",
    }});
    let [mill, last] = effects.as_mut_slice() else {
        return Vec::new();
    };
    if *last != kept || mill.pointer("/Mill/as") != Some(&json!("milled")) {
        return Vec::new();
    }
    if let Some(mill) = mill.get_mut("Mill").and_then(Value::as_object_mut) {
        mill.remove("as");
    }
    effects.pop();
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter_map(|text| {
            text.strip_suffix('.').map(|head| {
                format!(
                    "{head}. Put all creature cards with activated abilities that aren't mana abilities from among \
                     the milled cards into your hand."
                )
            })
        })
        .collect()
}

/// "Rather than pay the mana cost of the next spell you cast this turn, you may pay life equal to that spell's mana
/// value.": a `NextSpell` rule until end of turn whose spell may be cast for life equal to its mana value; the twin draws
/// a card.
fn life_rather(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let rule = json!([{"Rule": {
        "rule": {"NextSpell": {"benefit": {"Rather": [{"PayLife": {"Stat": {"of": "This", "stat": "ManaValue"}}}]}}},
        "duration": "UntilEndOfTurn",
    }}]);
    let Some(slot) = value.pointer_mut("/Activated/effects") else {
        return Vec::new();
    };
    if *slot != rule {
        return Vec::new();
    }
    *slot = json!([{"Draw": {}}]);
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter(|text| text.ends_with(": Draw a card."))
        .map(|text| {
            text.replace(
                ": Draw a card.",
                ": Rather than pay the mana cost of the next spell you cast this turn, you may pay life equal to that \
                 spell's mana value.",
            )
        })
        .collect()
}

/// "Proliferate X times, where X is the number of planeswalker types among planeswalkers you control.": a proliferate
/// repeated as many times as there are different subtypes among the planeswalkers the controller controls; the twin
/// proliferates once.
fn proliferate_per_type(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let repeat = json!([{"RepeatWhile": {
        "condition": {"Compare": {"left": 0, "op": "Eq", "right": 0}},
        "effects": [{"Proliferate": {}}],
        "most": crate::effect::MOST_PASSES,
        "times": {"Distinct": {"of": [{"Type": "Planeswalker"}, {"ControlledBy": "You"}], "stat": "Subtypes"}},
    }}]);
    let Some(slot) = value.pointer_mut("/Activated/effects") else {
        return Vec::new();
    };
    if *slot != repeat {
        return Vec::new();
    }
    *slot = json!([{"Proliferate": {}}]);
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter(|text| text.ends_with(": Proliferate."))
        .map(|text| {
            text.replace(
                ": Proliferate.",
                ": Proliferate X times, where X is the number of planeswalker types among planeswalkers you control.",
            )
        })
        .collect()
}

/// "Look at the top eight cards of your library. Exile four of them at random, then put the rest on top of your library
/// in any order.": a look, a random pick among the cards looked at, their exile and the others put back on top in an
/// order their owner chooses; the twin draws a card.
fn exiled_at_random(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let Some(Value::Array(effects)) = value.pointer("/Activated/effects") else {
        return Vec::new();
    };
    let [look, pick, exile, rest] = effects.as_slice() else {
        return Vec::new();
    };
    let (Some(seen), Some(taken)) = (
        look.pointer("/LookAtTop/count").and_then(Value::as_u64),
        pick.pointer("/ChooseAtRandom/count")
            .and_then(Value::as_u64),
    ) else {
        return Vec::new();
    };
    let shape = json!([
        {"LookAtTop": {"count": seen, "as": "top"}},
        {"ChooseAtRandom": {"from": {"Bound": "top"}, "count": taken, "as": "exiled"}},
        {"Exile": {"what": {"Bound": "exiled"}}},
        {"Move": {
            "what": {"Each": [{"In": "Library"}, {"Is": {"Bound": "top"}}]},
            "to": {"Library": {"place": "Top", "order": "Chosen"}},
        }},
    ]);
    if json!([look, pick, exile, rest]) != shape || taken >= seen {
        return Vec::new();
    }
    let (Some(seen), Some(taken)) = (
        u32::try_from(seen).ok().and_then(count_word),
        u32::try_from(taken).ok().and_then(count_word),
    ) else {
        return Vec::new();
    };
    let Some(slot) = value.pointer_mut("/Activated/effects") else {
        return Vec::new();
    };
    *slot = json!([{"Draw": {}}]);
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    let words = format!(
        ": Look at the top {seen} cards of your library. Exile {taken} of them at random, then put the rest on top of \
         your library in any order."
    );
    written(&twin)
        .into_iter()
        .filter(|text| text.ends_with(": Draw a card."))
        .map(|text| text.replace(": Draw a card.", &words))
        .collect()
}

/// "{3}, {T}: Return target artifact creature card from your graveyard to your hand.": an activated return of one target
/// card from a graveyard to its owner's hand, worded as the spell of the same target and step after the cost; the twins
/// are that spell and the ability drawing a card.
fn returned_from_graveyard(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let Some(body) = value.get("Activated").and_then(Value::as_object) else {
        return Vec::new();
    };
    let step = json!([{"Move": {"what": {"Target": 0}, "to": "Hand"}}]);
    let Some(Value::Array(targets)) = body.get("targets") else {
        return Vec::new();
    };
    let [target] = targets.as_slice() else {
        return Vec::new();
    };
    let from_graveyard = target
        .pointer("/what/Object")
        .and_then(Value::as_array)
        .is_some_and(|preds| preds.contains(&json!({"In": "Graveyard"})));
    if body.get("effects") != Some(&step) || !from_graveyard {
        return Vec::new();
    }
    // "target artifact creature card" is worded from the twin of a plain creature card.
    let artifact_creature = target.pointer("/what/Object")
        == Some(
            &json!([{"Type": "Artifact"}, {"Type": "Creature"}, {"In": "Graveyard"}, {"OwnedBy": "You"}]),
        );
    let spell_targets = if artifact_creature {
        json!([{"what": {"Object": [{"Type": "Creature"}, {"In": "Graveyard"}, {"OwnedBy": "You"}]}}])
    } else {
        json!(targets)
    };
    let spell = json!({"Spell": {"targets": spell_targets, "effects": step}});
    let mut drawing = value.clone();
    if let Some(slot) = drawing.pointer_mut("/Activated/effects") {
        *slot = json!([{"Draw": {}}]);
    }
    if let Some(Value::Object(body)) = drawing.get_mut("Activated") {
        body.remove("targets");
    }
    let (Ok(spell), Ok(drawing)) = (
        serde_json::from_value::<Ability>(spell),
        serde_json::from_value::<Ability>(drawing),
    ) else {
        return Vec::new();
    };
    let heads: Vec<String> = written(&drawing)
        .iter()
        .filter_map(|text| text.strip_suffix(": Draw a card.").map(str::to_owned))
        .collect();
    let mut texts = Vec::new();
    for head in &heads {
        for words in written(&spell) {
            let words = if artifact_creature {
                words.replacen("target creature card", "target artifact creature card", 1)
            } else {
                words
            };
            texts.push(format!("{head}: {words}"));
        }
    }
    texts
}

/// Both predicates, as `AnyOf` and `Not` say it: neither is false.
fn both(first: Value, second: Value) -> Value {
    json!({"Not": {"AnyOf": [{"Not": first}, {"Not": second}]}})
}

/// "Search your library for an artifact card with a mana ability or a basic land card, reveal it, ...": a search whose
/// one filter is an artifact with a mana ability or a basic land; the twin searches for an artifact card.
fn mana_artifact_or_basic(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let wanted = json!([{"AnyOf": [
        both(json!({"Type": "Artifact"}), json!({"HasAbility": "Mana"})),
        both(json!({"Type": "Land"}), json!({"Supertype": "Basic"})),
    ]}]);
    let Some(filter) = value.pointer_mut("/Activated/effects/0/Search/filter") else {
        return Vec::new();
    };
    if *filter != wanted {
        return Vec::new();
    }
    *filter = json!([{"Type": "Artifact"}]);
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter(|text| text.contains("Search your library for an artifact card, "))
        .map(|text| {
            text.replacen(
                "Search your library for an artifact card, ",
                "Search your library for an artifact card with a mana ability or a basic land card, ",
                1,
            )
        })
        .collect()
}
