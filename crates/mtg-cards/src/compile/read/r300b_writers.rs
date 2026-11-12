//! Wordings of pack r30-0b: activated abilities whose condition, mana rider or steps no other writer words, each written
//! from a twin the other writers word with that one part changed.

use std::sync::OnceLock;

use serde_json::{Value, json};

use super::tokens::Reader;
use crate::effect::Effect;
use crate::macros;
use crate::refs::Ref;

use super::amounts::count_word;
use crate::ability::Ability;
use crate::refs::Selector;
use crate::value::StatKind;

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

/// "color among permanents you control": the colors among the objects, for `Amount::Distinct` over colors.
pub(super) fn r300b_distinct_colors_text(
    of: &Selector,
    stat: StatKind,
    plural: bool,
) -> Option<String> {
    (stat == StatKind::Colors).then_some(())?;
    let name = if plural { "colors" } else { "color" };
    Some(format!(
        "{name} among {}",
        super::defining::counted_text(of)?
    ))
}

/// Every wording this pack adds for `ability`.
pub(super) fn r300b_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Some(value) = value_of(ability) else {
        return;
    };
    let mut found = Vec::new();
    if let Some(body) = value.get("Activated") {
        found.extend(upkeep_above(body, written));
        found.extend(spell_steps(body, written, texts.is_empty()));
        found.extend(owner_passes(body, written));
        found.extend(commander_mana(body, written));
        found.extend(owner_only(body, written));
        found.extend(redirect_to_owner(body, written));
        found.extend(excess_draws(body, written));
        found.extend(balloon_copy(body, written));
        found.extend(search_become_copy(body, written));
        found.extend(owner_also_draws(body, written));
        found.extend(remove_one_or_more(body, written));
    }
    if let Some(cost) = value["Keyword"]["MoreThanMeetsTheEye"].as_str() {
        found.push(format!("More Than Meets the Eye {cost}"));
    }
    if let Some(body) = value.get("Triggered") {
        found.extend(own_hand_shuffled(body, written));
        found.extend(owner_loses_half(body, written));
    }
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// The words of an activated ability up to and including its colon ("{5}, {T}: "), from its twin that draws a card with
/// no target, condition or timing.
fn cost_head(body: &Value, written: fn(&Ability) -> Vec<String>) -> Option<String> {
    let mut twin = body.clone();
    let map = twin.as_object_mut()?;
    for key in ["targets", "condition", "timing", "effects"] {
        map.remove(key);
    }
    map.insert("effects".to_owned(), json!([{"Draw": {}}]));
    let twin = ability_of(json!({ "Activated": twin }))?;
    written(&twin)
        .into_iter()
        .find_map(|text| text.strip_suffix("Draw a card.").map(str::to_owned))
}

/// "Activate only during your upkeep and only if three or more creature cards are above this card.": the return from a
/// graveyard written from its twin that asks for a Swamp, the condition's last part changed.
fn upkeep_above(body: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    upkeep_above_text(body, written)
        .map(|text| vec![text.replace("this card.", "~."), text])
        .unwrap_or_default()
}

/// [`upkeep_above`] with the card called "this card".
fn upkeep_above_text(body: &Value, written: fn(&Ability) -> Vec<String>) -> Option<String> {
    let [your_turn, upkeep, compare] = body["condition"]["All"].as_array()?.as_slice() else {
        return None;
    };
    (*your_turn == json!("YourTurn") && *upkeep == json!({"Step": "Upkeep"})).then_some(())?;
    let above = &compare["Compare"];
    (above["op"] == json!("Ge")).then_some(())?;
    let least = number(&above["right"])?;
    let counted = &above["left"]["CardsAbove"];
    (counted.get("directly").is_none_or(|d| *d == json!(false))).then_some(())?;
    let filter: Selector = serde_json::from_value(counted["filter"].clone()).ok()?;
    let kind = super::selectors::phrase_text(&filter, super::selectors::Form::All)?;
    let kind = kind.strip_suffix('s').unwrap_or(&kind).to_owned();
    let mut twin = body.clone();
    twin["condition"] = json!({"All": ["YourTurn", {"Step": "Upkeep"}, {"Compare": {
        "left": {"Count": [{"Subtype": "Swamp"}, {"ControlledBy": "You"}]}, "op": "Ge", "right": 1
    }}]});
    let twin = ability_of(json!({ "Activated": twin }))?;
    written(&twin).into_iter().find_map(|text| {
        let rest = text.strip_suffix(" and only if you control a Swamp.")?;
        Some(format!(
            "{rest} and only if {least} or more {kind} cards are above this card."
        ))
    })
}

/// "{R}, Sacrifice this creature: Each player shuffles ...": an activated ability with no target whose one step is done
/// for each player, written as the cost and the spell's text; only for an ability nothing else words.
fn spell_steps(body: &Value, written: fn(&Ability) -> Vec<String>, unwritten: bool) -> Vec<String> {
    let each_player = body["effects"].as_array().is_some_and(|steps| {
        steps.len() == 1 && steps[0]["RepeatEach"]["over"] == json!("EachPlayer")
    });
    let plain = unwritten
        && each_player
        && ["targets", "condition", "timing", "limit", "by", "from"]
            .iter()
            .all(|key| body.get(*key).is_none());
    if !plain {
        return Vec::new();
    }
    let Some(head) = cost_head(body, written) else {
        return Vec::new();
    };
    let Some(spell) = ability_of(json!({"Spell": {"effects": body["effects"].clone()}})) else {
        return Vec::new();
    };
    written(&spell)
        .into_iter()
        .map(|text| format!("{head}{text}"))
        .collect()
}

/// "Target player other than this artifact's owner gains control of it. Activate only during your turn.": control of
/// the source passes for good to a target player who does not own it.
fn owner_passes(body: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let player = json!([{"what": {"PlayerAmong": {"PlayersWhere": {"who": "EachPlayer", "test": {"Not": {
        "Matches": {"subject": "This", "selector": [{"OwnedBy": "Asker"}]}
    }}}}}}]);
    let passes = json!([{"Continuous": {
        "affected": "This", "mods": [{"SetController": {"Target": 0}}], "duration": "Forever"
    }}]);
    if body["targets"] != player || body["effects"] != passes || body.get("timing").is_some() {
        return Vec::new();
    }
    let tail = match body.get("condition") {
        None => "",
        Some(condition) if *condition == json!("YourTurn") => " Activate only during your turn.",
        Some(_) => return Vec::new(),
    };
    let Some(head) = cost_head(body, written) else {
        return Vec::new();
    };
    ["this artifact's", "~'s"]
        .iter()
        .map(|owner| {
            format!("{head}Target player other than {owner} owner gains control of it.{tail}")
        })
        .collect()
}

/// "When you spend this mana to cast your commander, scry X, where X is the number of times it's been cast from the
/// command zone this game." and "If you spend this mana to cast your commander, it enters with a number of additional
/// +1/+1 counters on it equal to ...": a mana ability's rider about the commander, after the twin's mana.
fn commander_mana(body: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    const TIMES: &str = "the number of times it's been cast from the command zone this game";
    let [add] = body["effects"].as_array().map_or(&[][..], Vec::as_slice) else {
        return Vec::new();
    };
    let mana = &add["AddMana"];
    let commander = json!({"facts": ["YourCommander"]});
    let scry = json!({"spell": commander, "effects": [{"Scry": {"count": {"CastsFromCommand": "Triggering"}}}]});
    let enters = json!([{"Enters": {"spell": commander, "per_command_cast": true}}]);
    let rider = if mana.get("when_spent") == Some(&scry) && mana.get("only").is_none() {
        format!("When you spend this mana to cast your commander, scry X, where X is {TIMES}.")
    } else if mana.get("only") == Some(&enters) && mana.get("when_spent").is_none() {
        format!(
            "If you spend this mana to cast your commander, it enters with a number of additional +1/+1 counters on it equal to {TIMES}."
        )
    } else {
        return Vec::new();
    };
    let mut twin = body.clone();
    if let Some(map) = twin["effects"][0]["AddMana"].as_object_mut() {
        map.remove("when_spent");
        map.remove("only");
    }
    let Some(twin) = ability_of(json!({ "Activated": twin })) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .map(|text| format!("{text} {rider}"))
        .collect()
}

/// "When this creature enters, shuffle the cards from your hand into your library, then draw that many cards.": the
/// hand counted, shuffled in and drawn again.
fn own_hand_shuffled(body: &Value, written: fn(&Ability) -> Vec<String>) -> Option<String> {
    let hand = json!([{"In": "Hand"}, {"OwnedBy": "You"}]);
    let steps = json!([
        {"Let": {"as": "n", "amount": {"Count": hand}}},
        {"Move": {"what": {"Each": hand}, "to": {"Library": {"place": "Shuffled"}}}},
        {"Draw": {"count": {"Bound": "n"}}}
    ]);
    (body["effects"] == steps && body.get("targets").is_none()).then_some(())?;
    let mut twin = body.clone();
    twin["effects"] = json!([{"Draw": {}}]);
    let twin = ability_of(json!({ "Triggered": twin }))?;
    written(&twin).into_iter().find_map(|text| {
        let head = text.strip_suffix(" draw a card.")?;
        Some(format!(
            "{head} shuffle the cards from your hand into your library, then draw that many cards."
        ))
    })
}

/// "Only this creature's owner may activate this ability.": the words of the twin its controller activates, then the
/// sentence.
fn owner_only(body: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    if body.get("by") != Some(&json!("Owner")) {
        return Vec::new();
    }
    let mut twin = body.clone();
    if let Some(map) = twin.as_object_mut() {
        map.remove("by");
    }
    let Some(twin) = ability_of(json!({ "Activated": twin })) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .flat_map(|text| {
            ["this creature's", "~'s"]
                .map(|owner| format!("{text} Only {owner} owner may activate this ability."))
        })
        .collect()
}

/// "The next 1 damage that would be dealt to this creature this turn is dealt to its owner instead.": written from the
/// twin that redirects it to target creature you control.
fn redirect_to_owner(body: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let [shield] = body["effects"].as_array().map_or(&[][..], Vec::as_slice) else {
        return Vec::new();
    };
    let redirect = &shield["Shield"]["by"]["RedirectUpTo"];
    if redirect["to"] != json!({"OwnerOf": "This"}) || body.get("targets").is_some() {
        return Vec::new();
    }
    let mut twin = body.clone();
    twin["effects"][0]["Shield"]["by"]["RedirectUpTo"]["to"] = json!({"Target": 0});
    twin["targets"] =
        json!([{"what": {"Object": [{"Type": "Creature"}, {"ControlledBy": "You"}]}}]);
    let Some(twin) = ability_of(json!({ "Activated": twin })) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter_map(|text| {
            text.contains(" is dealt to target creature you control instead")
                .then(|| {
                    text.replace(
                        " is dealt to target creature you control instead",
                        " is dealt to its owner instead",
                    )
                })
        })
        .collect()
}

/// "If excess damage was dealt to a creature this way, draw a card.": after the twin that deals the damage with no
/// excess kept.
fn excess_draws(body: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let [deal, check] = body["effects"].as_array().map_or(&[][..], Vec::as_slice) else {
        return Vec::new();
    };
    let to = &deal["DealDamage"]["to"];
    let draws = json!({"If": {"condition": {"All": [
        {"Compare": {"left": {"Bound": "excess"}, "op": "Ge", "right": 1}},
        {"Matches": {"subject": to, "selector": [{"Type": "Creature"}]}}
    ]}, "then": [{"Draw": {}}]}});
    if deal["DealDamage"]["excess"] != json!({"as": "excess"}) || *check != draws {
        return Vec::new();
    }
    let mut twin = body.clone();
    twin["effects"] = json!([deal.clone()]);
    if let Some(map) = twin["effects"][0]["DealDamage"].as_object_mut() {
        map.remove("excess");
    }
    let Some(twin) = ability_of(json!({ "Activated": twin })) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .map(|text| {
            format!("{text} If excess damage was dealt to a creature this way, draw a card.")
        })
        .collect()
}

/// "except it's a 1/1 red Balloon creature in addition to its other colors and types and it has flying and haste.
/// Sacrifice it at the beginning of the next end step.": a token copy given a color and keywords too, sacrificed at the
/// next end step, written from the twin without the color, the keywords and the sacrifice.
fn balloon_copy(body: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let [create, delayed] = body["effects"].as_array().map_or(&[][..], Vec::as_slice) else {
        return Vec::new();
    };
    let sacrifice = json!({"DelayedTrigger": {"when": {"StepBegins": {"step": "End"}}, "effects": [
        {"Sacrifice": {"filter": [{"Is": {"Bound": "copy"}}]}}
    ]}});
    let made = &create["CreateToken"];
    if *delayed != sacrifice || made["as"] != json!("copy") {
        return Vec::new();
    }
    let Some(except) = made["token"]["CopyOf"]["except"].as_array() else {
        return Vec::new();
    };
    let mut colors = Vec::new();
    let mut keywords = Vec::new();
    let mut kept = Vec::new();
    for change in except {
        if let Some(added) = change["AddColors"].as_array() {
            colors.extend(
                added
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_lowercase),
            );
        } else if let Some(keyword) = change["AddKeyword"].as_str() {
            keywords.push(keyword.to_lowercase());
        } else {
            kept.push(change.clone());
        }
    }
    let ([color], [first, second]) = (colors.as_slice(), keywords.as_slice()) else {
        return Vec::new();
    };
    let mut twin = body.clone();
    let timing = twin.as_object_mut().and_then(|map| map.remove("timing"));
    let tail = match timing {
        None => "",
        Some(Value::String(timing)) if timing == "Sorcery" => " Activate only as a sorcery.",
        Some(_) => return Vec::new(),
    };
    twin["effects"] = json!([{"CreateToken": {"token": {"CopyOf": {
        "of": made["token"]["CopyOf"]["of"].clone(), "except": kept
    }}}}]);
    let Some(twin) = ability_of(json!({ "Activated": twin })) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter_map(|text| {
            let (head, rest) = text.split_once(", except it's a ")?;
            let (size, kind) = rest.split_once(' ')?;
            let kind = kind.strip_suffix(" creature in addition to its other types.")?;
            Some(format!(
                "{head}, except it's a {size} {color} {kind} creature in addition to its other colors and types and it has {first} and {second}. Sacrifice it at the beginning of the next end step.{tail}"
            ))
        })
        .collect()
}

/// "When this creature dies, its owner loses half their life, rounded up.": the twin in which you lose half your life.
fn owner_loses_half(body: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let owner = json!({"OwnerOf": "This"});
    let loses = json!([{"LoseLife": {"who": owner, "amount": {"Half": {"of": {"Life": owner}, "up": true}}}}]);
    if body["effects"] != loses || body.get("targets").is_some() {
        return Vec::new();
    }
    let mut twin = body.clone();
    twin["effects"] =
        json!([{"LoseLife": {"amount": {"Half": {"of": {"Life": "You"}, "up": true}}}}]);
    let Some(twin) = ability_of(json!({ "Triggered": twin })) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter_map(|text| {
            let head = text.strip_suffix(" you lose half your life, rounded up.")?;
            Some(format!(
                "{head} its owner loses half their life, rounded up."
            ))
        })
        .collect()
}

/// The most Jace loyalty an empower is matched back to words for.
const EMPOWER_MOST: u32 = 20;

/// "empower Jace 3": the steps of the macro table's `empower` with the number.
pub(super) fn r300b_read_empower(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.attempt(|r| {
        (r.word("empower") && r.word("jace")).then_some(())?;
        let amount = r.number().filter(|n| *n > 0)?;
        macros::action_effects("empower", amount, &Ref::You)
    })
}

/// The words "empower Jace N" of the effects an empower begins with, and how many effects it is.
pub(super) fn r300b_empower_text(effects: &[Effect]) -> Option<(String, usize)> {
    static CACHE: OnceLock<Vec<(u32, Vec<Effect>)>> = OnceLock::new();
    let rows = CACHE.get_or_init(|| {
        (1..=EMPOWER_MOST)
            .filter_map(|n| Some((n, macros::action_effects("empower", n, &Ref::You)?)))
            .collect()
    });
    rows.iter().find_map(|(n, steps)| {
        effects
            .starts_with(steps)
            .then(|| (format!("empower Jace {n}"), steps.len()))
    })
}

/// "Search your library for a Dragon permanent card and put it into your graveyard. If you do, ~ becomes a copy of that
/// card until end of turn. Then shuffle.": the search written from its twin without the copy and the shuffle.
fn search_become_copy(body: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let [search, moved, copy, shuffle] = body["effects"].as_array().map_or(&[][..], Vec::as_slice)
    else {
        return Vec::new();
    };
    let found = json!({"Bound": "found"});
    let becomes = json!({"If": {
        "condition": {"CountOf": {"subject": found, "selector": [{"In": "Graveyard"}], "op": "Ge", "right": 1}},
        "then": [{"BecomeCopy": {"what": "This", "of": found, "duration": "UntilEndOfTurn"}}]
    }});
    if search["Search"]["as"] != json!("found")
        || *moved != json!({"Move": {"what": found, "to": "Graveyard"}})
        || *copy != becomes
        || *shuffle != json!({"Shuffle": {}})
        || body.get("targets").is_some()
    {
        return Vec::new();
    }
    let mut twin = body.clone();
    twin["effects"] = json!([search.clone(), moved.clone(), shuffle.clone()]);
    let Some(twin) = ability_of(json!({ "Activated": twin })) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter_map(|text| {
            let head = text.strip_suffix(", put it into your graveyard, then shuffle.")?;
            Some(format!(
                "{head} and put it into your graveyard. If you do, ~ becomes a copy of that card until end of turn. Then shuffle."
            ))
        })
        .collect()
}

/// "Draw a card, then you may put a land card from your hand onto the battlefield." for a player, the land bound as `bind`:
/// its JSON.
fn draw_then_land(who: &Value, bind: &str) -> Value {
    let mut draw = json!({"Draw": {}});
    let mut choose = json!({"Choose": {"from": {"Each": [{"In": "Hand"}, {"OwnedBy": who}, {"Type": "Land"}]}, "as": bind}});
    let mut may = json!({"May": {"effects": []}});
    if *who != json!("You") {
        draw["Draw"]["who"] = who.clone();
        choose["Choose"]["who"] = who.clone();
        may["May"]["who"] = who.clone();
    }
    may["May"]["effects"] =
        json!([choose, {"Move": {"what": {"Bound": bind}, "to": {"Battlefield": {}}}}]);
    json!([draw, may])
}

/// "This artifact's owner draws a card, then that player may put a land card from their hand onto the battlefield.":
/// after the twin in which you do so.
fn owner_also_draws(body: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Some(steps) = body["effects"].as_array() else {
        return Vec::new();
    };
    let mine = draw_then_land(&json!("You"), "card");
    let owners = draw_then_land(&json!({"OwnerOf": "This"}), "owners_land");
    let (Some(mine), Some(owners)) = (mine.as_array(), owners.as_array()) else {
        return Vec::new();
    };
    if steps.len() != 4 || steps[..2] != mine[..] || steps[2..] != owners[..] {
        return Vec::new();
    }
    let mut twin = body.clone();
    twin["effects"] = json!(mine);
    let Some(twin) = ability_of(json!({ "Activated": twin })) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .flat_map(|text| {
            ["this artifact's", "~'s"].map(|owner| {
                format!("{text} {}{} owner draws a card, then that player may put a land card from their hand onto the battlefield.", owner[..1].to_uppercase(), &owner[1..])
            })
        })
        .collect()
}

/// "{2}{G}: Return this card from your graveyard or from exile to the battlefield tapped.": two activations equal but
/// for the zone each works from, a graveyard first and exile second, written from the graveyard's.
pub(super) fn r300b_two_zone_texts(first: &Ability, second: &Ability) -> Vec<String> {
    let (Some(graveyard), Some(exile)) = (value_of(first), value_of(second)) else {
        return Vec::new();
    };
    let (Some(from_graveyard), Some(from_exile)) =
        (graveyard.get("Activated"), exile.get("Activated"))
    else {
        return Vec::new();
    };
    let mut same = from_exile.clone();
    same["from"] = json!("Graveyard");
    if from_graveyard["from"] != json!("Graveyard")
        || from_exile["from"] != json!("Exile")
        || same != *from_graveyard
    {
        return Vec::new();
    }
    super::render::written_variants(first)
        .into_iter()
        .filter(|text| !text.contains("ctivate only"))
        .filter_map(|text| {
            text.contains(" from your graveyard to the battlefield")
                .then(|| {
                    text.replacen(
                        " from your graveyard to the battlefield",
                        " from your graveyard or from exile to the battlefield",
                        1,
                    )
                })
        })
        .collect()
}

/// "{1}, Remove one or more +1/+1 counters from ~: It deals that much damage to target creature. Convert ~.": X counters
/// removed, at least one, written from the twin that removes X and deals X without converting.
fn remove_one_or_more(body: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Some(cost) = body["cost"].as_array() else {
        return Vec::new();
    };
    let Some(effects) = body["effects"].as_array() else {
        return Vec::new();
    };
    let (Some((last_cost, cost_rest)), Some((last_step, steps))) =
        (cost.split_last(), effects.split_last())
    else {
        return Vec::new();
    };
    let removes_x = cost_rest
        .iter()
        .any(|part| part["RemoveCounters"]["count"] == json!("X"));
    let deals_x = steps
        .iter()
        .any(|step| step["DealDamage"]["amount"] == json!("X"));
    if *last_cost != json!({"XAtLeast": 1})
        || *last_step != json!({"Transform": {"what": "This"}})
        || !removes_x
        || !deals_x
    {
        return Vec::new();
    }
    let mut twin = body.clone();
    twin["cost"] = json!(cost_rest);
    twin["effects"] = json!(steps);
    let Some(twin) = ability_of(json!({ "Activated": twin })) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter(|text| text.contains("Remove X ") && text.contains(" deals X damage "))
        .map(|text| {
            let text = text
                .replacen("Remove X ", "Remove one or more ", 1)
                .replacen(" deals X damage ", " deals that much damage ", 1);
            format!("{text} Convert ~.")
        })
        .collect()
}
