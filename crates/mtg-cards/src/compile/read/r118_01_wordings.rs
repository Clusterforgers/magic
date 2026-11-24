//! Wordings of the Star Trek leftovers of r118-01: a player who can't attack for a while (`Prohibited::R118Attack`: "they
//! can't attack this turn", "can't attack you during their next turn"), an optional step of the player a trigger names
//! with what follows if they do ("that player may draw a card. If they do, they can't attack this turn and you put a
//! peace counter on this enchantment.") and the beginning of combat said "combat on each player's turn".

use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::{Duration, GameRule, Prohibited};
use crate::refs::Ref;
use crate::trigger::{Step, TriggerPattern};
use crate::value::Condition;

/// The head of a trigger at each beginning of combat as the writers word it.
const EACH_COMBAT: &str = "At the beginning of each combat,";

/// The same head as the Star Trek set words it.
const EACH_PLAYERS_COMBAT: &str = "At the beginning of combat on each player's turn,";

/// "they can't attack this turn", "that player can't attack you during their next turn": a player's ban on attacking,
/// with the player as `subject` words them.
pub(super) fn r118_cant_attack_text(effect: &Effect, subject: &str) -> Option<String> {
    let Effect::Rule {
        rule:
            GameRule::Cant {
                who,
                action: Prohibited::R118Attack { you, walkers },
            },
        duration,
    } = effect
    else {
        return None;
    };
    if !is_player(who) {
        return None;
    }
    match (you, walkers, duration) {
        (false, false, Duration::UntilEndOfTurn) => Some(format!("{subject} can't attack this turn")),
        (true, true, Duration::UntilEndOfTurn) => Some(format!(
            "{subject} can't attack you or planeswalkers you control this turn"
        )),
        (true, false, Duration::TheirNextTurn) => {
            Some(format!("{subject} can't attack you during their next turn"))
        }
        _ => None,
    }
}

/// Whether the reference is one player a trigger or a step names.
fn is_player(who: &Ref) -> bool {
    matches!(
        who,
        Ref::Triggering | Ref::TriggeringPlayer | Ref::Target(0) | Ref::Bound(_)
    )
}

/// A writer of whole abilities from their twins.
type Twin = fn(&Ability, fn(&Ability) -> Vec<String>) -> Vec<String>;

/// The writers of abilities in the Star Trek wordings, each from a twin another writer words.
const R118_TWINS: [Twin; 15] = [
    counters_on_two_targets,
    exiled_until_by_target,
    permanents_exiled_this_way,
    revealed_until_several,
    one_or_two,
    type_or_subtype,
    doubled,
    hand_for_hand,
    instead_by_power,
    if_x_is,
    milled_to_hand,
    entered_this_turn,
    entering_with_counters,
    owner_draws_then_shuffles,
    attacking_each_attacked,
];

/// Adds the r118-01 wordings of the ability.
pub(super) fn r118_01_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let mut found = may_other(ability, written);
    if let Ability::Triggered {
        when:
            TriggerPattern::StepBegins {
                step: Step::BeginCombat,
                whose: Ref::EachPlayer,
            },
        ..
    } = ability
    {
        let renamed: Vec<String> = texts
            .iter()
            .chain(&found)
            .filter_map(|text| text.strip_prefix(EACH_COMBAT))
            .map(|rest| format!("{EACH_PLAYERS_COMBAT}{rest}"))
            .collect();
        found.extend(renamed);
    }
    found.extend(counted_in_words(texts));
    found.extend(exiled_free_cast(ability, written));
    found.extend(chosen_per_opponent(ability, written));
    found.extend(each_player_may(ability, written));
    found.extend(whole_steps(ability, written));
    found.extend(no_maximum_until_next_turn(ability, written));
    found.extend(subtype_list(ability, written));
    found.extend(no_extra_turn_yet(ability, written));
    found.extend(phased_together(texts));
    for writer in R118_TWINS {
        found.extend(writer(ability, written));
    }
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// "Target opponent exiles the top X cards of their library, .... You may cast an instant or sorcery spell from among
/// them without paying its mana cost. Then that player puts the rest on the bottom of their library in a random order.":
/// the exile alone as its twin words it, then one free cast of a card of a kind among the exiled cards and the rest to the
/// bottom of the library they came from.
fn exiled_free_cast(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let Some(effects) = value
        .as_object_mut()
        .and_then(|map| map.values_mut().next())
        .and_then(|body| body.get_mut("effects"))
        .and_then(serde_json::Value::as_array_mut)
    else {
        return Vec::new();
    };
    let Some((spell, rest)) = free_cast_steps(effects) else {
        return Vec::new();
    };
    effects.truncate(1);
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    written(&twin)
        .iter()
        .filter(|text| text.ends_with('.') && text.contains(" exiles the top "))
        .map(|text| {
            format!(
                "{text} You may cast {} {spell} from among them without paying its mana cost. Then {rest}.",
                super::triggers::article(&spell)
            )
        })
        .collect()
}

/// The words of the free cast after an exile of the top cards of another player's library: the kind of spell and the
/// move of the rest.
fn free_cast_steps(effects: &[serde_json::Value]) -> Option<(String, String)> {
    use serde_json::json;
    let [exile, choose, cast, moved] = effects else {
        return None;
    };
    let exile = exile.get("ExileTop")?;
    let bind = exile.get("as")?.as_str()?;
    (exile.get("who")? == &json!({ "Target": 0 })).then_some(())?;
    let choose = choose.get("Choose")?;
    (choose.get("who").is_none() && choose.get("count")? == &json!({ "min": 0, "max": 1 }))
        .then_some(())?;
    let picked = choose.get("as")?.as_str()?;
    let among = [json!({ "Is": { "Bound": bind } }), json!({ "In": "Exile" })];
    let from = choose.get("from")?.get("Each")?.as_array()?;
    let kind: Vec<serde_json::Value> = from
        .iter()
        .filter(|pred| !among.contains(pred))
        .cloned()
        .collect();
    (from.len() == kind.len() + 2).then_some(())?;
    (cast == &json!({ "CastFromZone": { "what": { "Bound": picked }, "free": true } }))
        .then_some(())?;
    let bottom = json!({ "Move": {
        "what": { "Each": among },
        "to": { "Library": { "place": "Bottom", "order": "Random" } }
    } });
    (moved == &bottom).then_some(())?;
    let kind: crate::refs::Selector =
        serde_json::from_value(serde_json::Value::Array(kind)).ok()?;
    let card = super::library::filter_text(&kind, false)?;
    let spell = card.strip_suffix(" card")?;
    Some((
        format!("{spell} spell"),
        "that player puts the rest on the bottom of their library in a random order".to_owned(),
    ))
}

/// "Each opponent exiles the top X cards of their library, .... For each opponent, choose a land card that player owns
/// exiled this way. You may play the chosen cards for as long as they remain exiled.": the exile alone as its twin words
/// it, a choice among each opponent's exiled cards gathered into one name, and a permission to play them from exile.
fn chosen_per_opponent(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let Some(effects) = value
        .as_object_mut()
        .and_then(|map| map.values_mut().next())
        .and_then(|body| body.get_mut("effects"))
        .and_then(serde_json::Value::as_array_mut)
    else {
        return Vec::new();
    };
    let Some(card) = per_opponent_steps(effects) else {
        return Vec::new();
    };
    effects.truncate(1);
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    written(&twin)
        .iter()
        .filter(|text| text.ends_with('.') && text.contains(" exiles the top "))
        .map(|text| {
            format!(
                "{text} For each opponent, choose {} {card} that player owns exiled this way. You may play the chosen cards for as long as they remain exiled.",
                super::triggers::article(&card)
            )
        })
        .collect()
}

/// The kind of card chosen for each opponent after an exile of the top cards of each opponent's library, with the
/// permission to play the chosen cards from exile for good.
fn per_opponent_steps(effects: &[serde_json::Value]) -> Option<String> {
    use serde_json::json;
    let [exile, repeat, rule] = effects else {
        return None;
    };
    let exile = exile.get("ExileTop")?;
    let bind = exile.get("as")?.as_str()?;
    (exile.get("who")? == &json!("Opponents")).then_some(())?;
    let repeat = repeat.get("RepeatEach")?;
    (repeat.get("over")? == &json!("Opponents")).then_some(())?;
    let player = repeat.get("as")?.as_str()?;
    let [choose, gather] = repeat.get("effects")?.as_array()?.as_slice() else {
        return None;
    };
    let choose = choose.get("Choose")?;
    (choose.get("who").is_none() && choose.get("count").is_none()).then_some(())?;
    let picked = choose.get("as")?.as_str()?;
    let among = [
        json!({ "OwnedBy": { "Bound": player } }),
        json!({ "Is": { "Bound": bind } }),
        json!({ "In": "Exile" }),
    ];
    let from = choose.get("from")?.get("Each")?.as_array()?;
    let kind: Vec<serde_json::Value> = from
        .iter()
        .filter(|pred| !among.contains(pred))
        .cloned()
        .collect();
    (from.len() == kind.len() + among.len()).then_some(())?;
    let gathered = gather.get("Bind")?;
    (gathered.get("what")? == &json!({ "Bound": picked }) && gathered.get("adds")? == &json!(true))
        .then_some(())?;
    let chosen = gathered.get("as")?.as_str()?;
    let permission = json!({ "Rule": {
        "rule": { "MayCast": { "from": "Exile", "only": { "Bound": chosen }, "play": true } },
        "duration": "Forever"
    } });
    (rule == &permission).then_some(())?;
    let kind: crate::refs::Selector =
        serde_json::from_value(serde_json::Value::Array(kind)).ok()?;
    super::library::filter_text(&kind, false)
}

/// "if ~ has twelve or more peace counters on it": the number of counters a condition asks for in words, as the card
/// prints it.
fn counted_in_words(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter_map(|text| {
            let (before, rest) = text.split_once("if ~ has ")?;
            let (number, after) = rest.split_once(" or more ")?;
            let word = super::amounts::count_word(number.parse().ok()?)?;
            Some(format!("{before}if ~ has {word} or more {after}"))
        })
        .collect()
}

/// "<head>, that player may draw a card. If they do, they can't attack this turn and you put a peace counter on ~." and
/// "<head>, target opponent may have you draw a card. If they don't, they lose 4 life and you surveil 1.": the optional
/// step of the player the trigger names or targets and the steps done if they do (or don't), each of theirs said with
/// "they" and each of yours with "you".
fn may_other(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ability::Triggered {
        when,
        intervening_if,
        limit,
        from,
        targets,
        effects,
    } = ability
    else {
        return Vec::new();
    };
    let [
        Effect::May {
            who,
            effects: optional,
        },
        Effect::If {
            condition,
            then,
            otherwise,
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    let (Some(chooser), Some(outcome)) = (chooser_text(who, targets), outcome_text(condition))
    else {
        return Vec::new();
    };
    // "that player" is the player whose step it is; another trigger names its player its own way ("that attacking
    // player").
    if chooser == "that player" && !matches!(when, TriggerPattern::StepBegins { .. }) {
        return Vec::new();
    }
    let ([step], true) = (optional.as_slice(), otherwise.is_empty()) else {
        return Vec::new();
    };
    // Their own step is worded as an order ("may draw a card"); yours as one they have you do ("may have you draw").
    let (yours, have) = match as_yours(step, who) {
        Some(yours) => (yours, ""),
        None if matches!(who, Ref::Target(_)) => (step.clone(), "have you "),
        None => return Vec::new(),
    };
    let Some(done) = then
        .iter()
        .map(|effect| done_text(effect, who, written))
        .collect::<Option<Vec<String>>>()
    else {
        return Vec::new();
    };
    let Some(imperative) = sentence(&yours, written) else {
        return Vec::new();
    };
    let twin = Ability::Triggered {
        when: when.clone(),
        intervening_if: intervening_if.clone(),
        limit: *limit,
        from: *from,
        targets: if matches!(who, Ref::Target(_)) {
            Vec::new()
        } else {
            targets.clone()
        },
        effects: vec![yours],
    };
    let tail = format!(", {imperative}.");
    let done = and_list(&done);
    written(&twin)
        .iter()
        .filter_map(|text| text.strip_suffix(&tail))
        .map(|head| {
            format!(
                "{head}, {chooser} may {have}{} {outcome}, {done}.",
                stopped(&imperative)
            )
        })
        .collect()
}

/// The player who may: "that player" of the trigger, or the one target player or opponent.
fn chooser_text(who: &Ref, targets: &[crate::refs::TargetSpec]) -> Option<&'static str> {
    use crate::refs::Targetable;
    match (who, targets) {
        (Ref::Triggering | Ref::TriggeringPlayer, []) => Some("that player"),
        (Ref::Target(0), [spec]) if spec.bounds.is_one() => match spec.what {
            Targetable::Opponent => Some("target opponent"),
            Targetable::Player => Some("target player"),
            _ => None,
        },
        _ => None,
    }
}

/// "If they do" after the step taken, "If they don't" after the step refused.
fn outcome_text(condition: &Condition) -> Option<&'static str> {
    match condition {
        Condition::Performed => Some("If they do"),
        Condition::Not(inner) if **inner == Condition::Performed => Some("If they don't"),
        _ => None,
    }
}

/// "<head>, each player may create a tapped land token named Sanctum .... Each opponent who does can't attack you during
/// their next turn.": an optional step of each player in turn, and a ban for the next turn of each player who takes it
/// (a ban on attacking you means nothing to you, so "each opponent who does" is the same).
fn each_player_may(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ability::Triggered {
        when,
        intervening_if,
        limit,
        from,
        targets,
        effects,
    } = ability
    else {
        return Vec::new();
    };
    let [
        Effect::RepeatEach {
            over: Ref::EachPlayer,
            bind,
            effects: pass,
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    let who = Ref::Bound(bind.clone());
    let [
        Effect::May {
            who: chooser,
            effects: optional,
        },
        Effect::If {
            condition: Condition::Performed,
            then,
            otherwise,
        },
    ] = pass.as_slice()
    else {
        return Vec::new();
    };
    let ([step], [ban], true, true) = (
        optional.as_slice(),
        then.as_slice(),
        otherwise.is_empty(),
        *chooser == who,
    ) else {
        return Vec::new();
    };
    let Effect::Rule {
        rule: GameRule::Cant { who: banned, .. },
        ..
    } = ban
    else {
        return Vec::new();
    };
    let (Some(yours), Some(done), true) = (
        as_yours(step, &who),
        r118_cant_attack_text(ban, "each opponent who does"),
        *banned == who,
    ) else {
        return Vec::new();
    };
    let Some(imperative) = sentence(&yours, written) else {
        return Vec::new();
    };
    let twin = Ability::Triggered {
        when: when.clone(),
        intervening_if: intervening_if.clone(),
        limit: *limit,
        from: *from,
        targets: targets.clone(),
        effects: vec![yours],
    };
    let tail = format!(", {imperative}.");
    let done = super::render::capitalize(&done);
    written(&twin)
        .iter()
        .filter_map(|text| text.strip_suffix(&tail))
        .map(|head| format!("{head}, each player may {} {done}.", stopped(&imperative)))
        .collect()
}

/// The step with the player it names made you, so that it is worded as an order: "draw a card".
fn as_yours(step: &Effect, who: &Ref) -> Option<Effect> {
    let mut value = serde_json::to_value(step).ok()?;
    let body = value
        .as_object_mut()?
        .values_mut()
        .next()?
        .as_object_mut()?;
    let named = serde_json::to_value(who).ok()?;
    if body.get("who") != Some(&named) {
        return None;
    }
    body.remove("who");
    serde_json::from_value(value).ok()
}

/// A step done if they do: theirs after "they", yours after "you".
fn done_text(effect: &Effect, who: &Ref, written: fn(&Ability) -> Vec<String>) -> Option<String> {
    if let Some(text) = r118_cant_attack_text(effect, "they") {
        return Some(text);
    }
    // A step of theirs: "they lose 4 life".
    if let Some(theirs) = as_yours(effect, who) {
        let text = sentence(&theirs, written)?;
        return Some(format!("they {text}"));
    }
    let value = serde_json::to_value(effect).ok()?;
    let named = serde_json::to_string(who).ok()?;
    // A step of yours names neither the player nor "you" as another.
    if value.to_string().contains(&named) {
        return None;
    }
    Some(format!("you {}", sentence(effect, written)?))
}

/// The one-sentence wording of a step alone, without its capital and period: "put a peace counter on ~".
fn sentence(effect: &Effect, written: fn(&Ability) -> Vec<String>) -> Option<String> {
    let spell = Ability::Spell {
        targets: Vec::new(),
        effects: vec![effect.clone()],
    };
    let texts: Vec<String> = written(&spell)
        .into_iter()
        .filter_map(|text| text.strip_suffix('.').map(str::to_owned))
        .filter(|text| !text.contains(". "))
        .collect();
    // An order ("Draw a card"), or a step of yours without its subject ("You lose 4 life").
    texts
        .iter()
        .find(|text| !text.starts_with("You "))
        .map(|text| lower_first(text))
        .or_else(|| {
            texts
                .iter()
                .find_map(|text| text.strip_prefix("You ").map(str::to_owned))
        })
}

/// The text with its first letter in lower case.
fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}

/// "a and b", "a, b, and c".
fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [first, second] => format!("{first} and {second}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// The heads of the ability as its writers word it before its effects: "{T}: " and "Whenever ~ deals combat damage to a
/// player, ", each with whether its first effect starts a sentence.
fn heads(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<(String, bool)> {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let Some(fields) = value
        .as_object_mut()
        .and_then(|map| map.values_mut().next())
        .and_then(serde_json::Value::as_object_mut)
    else {
        return Vec::new();
    };
    fields.insert("effects".to_owned(), serde_json::json!([{ "Draw": {} }]));
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for text in written(&twin) {
        let head = if let Some(head) = text.strip_suffix("Draw a card.") {
            (head.to_owned(), true)
        } else if let Some(head) = text.strip_suffix("draw a card.") {
            (head.to_owned(), false)
        } else {
            continue;
        };
        if !found.contains(&head) {
            found.push(head);
        }
    }
    found
}

/// The ability with its effects worded as `steps` after each of its heads.
fn after_heads(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    steps: &str,
) -> Vec<String> {
    heads(ability, written)
        .into_iter()
        .map(|(head, capital)| {
            let steps = if capital {
                super::render::capitalize(steps)
            } else {
                steps.to_owned()
            };
            format!("{head}{steps}")
        })
        .collect()
}

/// "Each opponent exiles a card from their graveyard. You may exile a card from your graveyard." and "return each
/// creature card you own exiled with it to the battlefield" (S.S. Botany Bay): steps worded whole after the head.
fn whole_steps(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    use serde_json::json;
    let Ok(serde_json::Value::Array(effects)) = serde_json::to_value(ability.effects()) else {
        return Vec::new();
    };
    let graveyard_of = |player: serde_json::Value| json!({ "Each": [{ "In": "Graveyard" }, { "OwnedBy": player }] });
    let each_opponent = json!({ "RepeatEach": { "over": "Opponents", "as": "player", "effects": [
        { "Choose": { "who": { "Bound": "player" }, "from": graveyard_of(json!({ "Bound": "player" })), "as": "theirs" } },
        { "Exile": { "what": { "Bound": "theirs" } } }
    ] } });
    let you_may = json!({ "May": { "effects": [
        { "Choose": { "from": graveyard_of(json!("You")), "as": "yours" } },
        { "Exile": { "what": { "Bound": "yours" } } }
    ] } });
    let returned = json!({ "Move": {
        "what": { "Each": [{ "Type": "Creature" }, { "OwnedBy": "You" }, { "ExiledWith": "This" }] },
        "to": { "Battlefield": {} }
    } });
    let steps = if effects == [each_opponent, you_may] {
        "each opponent exiles a card from their graveyard. You may exile a card from your graveyard."
    } else if effects == [returned] {
        "return each creature card you own exiled with it to the battlefield."
    } else {
        return Vec::new();
    };
    after_heads(ability, written, steps)
}

/// "... You have no maximum hand size until your next turn.": the steps before it as their twin words them.
fn no_maximum_until_next_turn(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
) -> Vec<String> {
    let Ability::Spell { targets, effects } = ability else {
        return Vec::new();
    };
    let Some((
        Effect::Rule {
            rule: GameRule::NoMaxHandSize(Ref::You),
            duration: Duration::UntilYourNextTurn,
        },
        before,
    )) = effects.split_last()
    else {
        return Vec::new();
    };
    if before.is_empty() {
        return Vec::new();
    }
    let twin = Ability::Spell {
        targets: targets.clone(),
        effects: before.to_vec(),
    };
    written(&twin)
        .iter()
        .filter(|text| text.ends_with('.'))
        .map(|text| format!("{text} You have no maximum hand size until your next turn."))
        .collect()
}

/// "Other Officers, Artificers, Doctors, Pilots, Scientists, and Spacecraft creatures you control get +2/+2.": a group of
/// three or more subtypes, written from the twin of the first alone.
fn subtype_list(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    use crate::refs::{Pred, Selector};
    let Ability::Static {
        affected: Ref::Each(Selector(preds)),
        ..
    } = ability
    else {
        return Vec::new();
    };
    let Some((at, names)) = preds.iter().enumerate().find_map(|(at, pred)| {
        let Pred::AnyOf(parts) = pred else {
            return None;
        };
        let names: Option<Vec<String>> = parts
            .iter()
            .map(|part| match part {
                Pred::Subtype(subtype) => Some(subtype.0.clone()),
                _ => None,
            })
            .collect();
        names
            .filter(|names| names.len() > 2)
            .map(|names| (at, names))
    }) else {
        return Vec::new();
    };
    let (Some((last, rest)), Some(first)) = (names.split_last(), names.first()) else {
        return Vec::new();
    };
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let Some(slot) = value.pointer_mut(&format!("/Static/affected/Each/{at}")) else {
        return Vec::new();
    };
    *slot = serde_json::json!({ "Subtype": first });
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    let plurals: Vec<String> = rest
        .iter()
        .map(|name| super::subtypes::plural_subtype(name))
        .collect();
    let group = format!("{}, and {last} creatures", plurals.join(", "));
    let one = format!("Other {first} creatures ");
    written(&twin)
        .iter()
        .filter_map(|text| text.strip_prefix(&one))
        .map(|rest| format!("Other {group} {rest}"))
        .collect()
}

/// "When ~ enters, it and up to one target creature you control phase out.": the card and its target phased out together,
/// from the twin that words the two steps apart.
fn phased_together(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter_map(|text| {
            let (head, rest) = text.split_once(", it phases out. Up to one target ")?;
            let what = rest.strip_suffix(" phase out.")?;
            Some(format!("{head}, it and up to one target {what} phase out."))
        })
        .collect()
}

/// "If you control a Whale and haven't begun an extra turn this game, take an extra turn after this one.": a condition of
/// yours joined with "you haven't begun an extra turn this game", from the twin under the first alone.
fn no_extra_turn_yet(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let not_begun = serde_json::json!({ "Not": { "R118BegunExtraTurn": "You" } });
    if !strip_not_begun(&mut value, &not_begun) {
        return Vec::new();
    }
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    written(&twin)
        .iter()
        .filter_map(|text| {
            let (before, rest) = text.split_once("If you ")?;
            let (held, after) = rest.split_once(", ")?;
            Some(format!(
                "{before}If you {held} and haven't begun an extra turn this game, {after}"
            ))
        })
        .collect()
}

/// Replaces the one condition `All[c, not_begun]` in the value with `c`; whether it did.
fn strip_not_begun(value: &mut serde_json::Value, not_begun: &serde_json::Value) -> bool {
    if let Some(serde_json::Value::Array(parts)) = value.get("All")
        && let [held, last] = parts.as_slice()
        && last == not_begun
    {
        *value = held.clone();
        return true;
    }
    match value {
        serde_json::Value::Object(fields) => fields
            .values_mut()
            .any(|inner| strip_not_begun(inner, not_begun)),
        serde_json::Value::Array(items) => items
            .iter_mut()
            .any(|item| strip_not_begun(item, not_begun)),
        _ => false,
    }
}

/// The ability with every target spec of one or two targets made one of up to two, as JSON text, when it has one.
fn up_to_two_twin(ability: &Ability) -> Option<Ability> {
    let json = serde_json::to_string(ability).ok()?;
    let one_or_two = r#""bounds":{"min":1,"max":2}"#;
    json.contains(one_or_two).then_some(())?;
    serde_json::from_str(&json.replace(one_or_two, r#""bounds":{"min":0,"max":2}"#)).ok()
}

/// "one or two target creatures each get +2/+0 and gain lifelink until end of turn", "Return one or two target creature
/// cards ...": one or two targets, from the twin of up to two (CR 115.1d).
fn one_or_two(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Some(twin) = up_to_two_twin(ability) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for text in written(&twin) {
        if text.matches("p to two target ").count() != 1 {
            continue;
        }
        let text = text
            .replacen("up to two target ", "one or two target ", 1)
            .replacen("Up to two target ", "One or two target ", 1);
        // "each get" says that the two get it apart.
        if let Some((before, after)) = text.split_once(" get ") {
            found.push(format!("{before} each get {after}"));
        }
        // "each with mana value 3 or less" says the same of each card.
        if let Some((before, after)) = text.split_once(" cards with ") {
            found.push(format!("{before} cards each with {after}"));
        }
        found.push(text);
    }
    found
}

/// "target creature or Spacecraft", "two target creature and/or Spacecraft cards": a target of a card type or a
/// subtype, from the twin that targets the card type alone.
fn type_or_subtype(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(json) = serde_json::to_string(ability) else {
        return Vec::new();
    };
    let Some(at) = json.find(r#"{"AnyOf":[{"Type":""#) else {
        return Vec::new();
    };
    let rest = &json[at..];
    let Some(kind) = rest
        .strip_prefix(r#"{"AnyOf":[{"Type":""#)
        .and_then(|rest| rest.split_once('"'))
        .map(|(kind, _)| kind)
    else {
        return Vec::new();
    };
    let opening = format!(r#"{{"AnyOf":[{{"Type":"{kind}"}},{{"Subtype":""#);
    let Some(subtype) = rest
        .strip_prefix(&opening)
        .and_then(|after| after.split_once(r#""}]}"#))
        .map(|(subtype, _)| subtype)
    else {
        return Vec::new();
    };
    let whole = format!(r#"{opening}{subtype}"}}]}}"#);
    // Only a target is "target creature or Spacecraft"; a count or a filter of the same kind is worded another way.
    let in_targets = json.contains(&format!(r#""what":{{"Object":[{whole}"#));
    if !in_targets || json.matches(&whole).count() != 1 {
        return Vec::new();
    }
    let single = format!(r#"{{"Type":"{kind}"}}"#);
    let Ok(twin) = serde_json::from_str::<Ability>(&json.replacen(&whole, &single, 1)) else {
        return Vec::new();
    };
    let kind = kind.to_lowercase();
    let mut found = Vec::new();
    for text in written(&twin) {
        let plural = format!("target {kind} cards");
        let single = format!("target {kind}");
        if text.contains(&plural) {
            found.push(text.replacen(&plural, &format!("target {kind} and/or {subtype} cards"), 1));
        } else if text.matches(&single).count() == 1 {
            found.push(text.replacen(&single, &format!("target {kind} or {subtype}"), 1));
        }
    }
    found
}

/// "double its power and toughness until end of turn" for the creature of the trigger (CR 701.10a: it gets +X/+Y, X and
/// Y its power and toughness).
fn doubled(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    use serde_json::json;
    let Ok(serde_json::Value::Array(effects)) = serde_json::to_value(ability.effects()) else {
        return Vec::new();
    };
    let double = json!({ "Continuous": {
        "affected": "Triggering",
        "mods": [{ "ModifyPT": {
            "power": { "Stat": { "of": "Triggering", "stat": "Power" } },
            "toughness": { "Stat": { "of": "Triggering", "stat": "Toughness" } }
        } }],
        "duration": "UntilEndOfTurn"
    } });
    if effects != [double] {
        return Vec::new();
    }
    after_heads(
        ability,
        written,
        "double its power and toughness until end of turn.",
    )
}

/// "you may discard your hand. If you do, draw a card for each card discarded this way."
fn hand_for_hand(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    use serde_json::json;
    let Ok(serde_json::Value::Array(effects)) = serde_json::to_value(ability.effects()) else {
        return Vec::new();
    };
    let shape = [
        json!({ "May": { "effects": [{ "Discard": {
            "count": { "Count": [{ "In": "Hand" }, { "OwnedBy": "You" }] },
            "as": "discarded"
        } }] } }),
        json!({ "If": { "condition": "Performed", "then": [{ "Draw": { "count": { "Bound": "discarded~moved" } } }] } }),
    ];
    if effects != shape {
        return Vec::new();
    }
    after_heads(
        ability,
        written,
        "you may discard your hand. If you do, draw a card for each card discarded this way.",
    )
}

/// "surveil 1. If her power is 4 or greater, instead draw a card.": a step, replaced by another while the card's power
/// is at least a number, said of the card as "her", "his" or "its".
fn instead_by_power(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let [
        Effect::If {
            condition:
                Condition::Compare {
                    left:
                        crate::value::Amount::Stat {
                            of: Ref::This,
                            stat: crate::value::StatKind::Power,
                        },
                    op: crate::value::Cmp::Ge,
                    right: crate::value::Amount::Fixed(least),
                },
            then,
            otherwise,
        },
    ] = ability.effects()
    else {
        return Vec::new();
    };
    let ([instead], [plain]) = (then.as_slice(), otherwise.as_slice()) else {
        return Vec::new();
    };
    let (Some(plain), Some(instead)) = (sentence(plain, written), sentence(instead, written))
    else {
        return Vec::new();
    };
    ["her", "his", "its"]
        .iter()
        .flat_map(|pronoun| {
            after_heads(
                ability,
                written,
                &format!("{plain}. If {pronoun} power is {least} or greater, instead {instead}."),
            )
        })
        .collect()
}

/// "If X is 6 or more, create two 2/1 red Klingon Warrior creature tokens with haste.": a step under a test of the X a
/// where clause of the sentence before defines.
fn if_x_is(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ability::Spell { targets, effects } = ability else {
        return Vec::new();
    };
    let [
        first,
        Effect::If {
            condition:
                Condition::Compare {
                    left,
                    op: crate::value::Cmp::Ge,
                    right: crate::value::Amount::Fixed(least),
                },
            then,
            otherwise,
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    let (Ok(first_json), Ok(left_json)) =
        (serde_json::to_string(first), serde_json::to_string(left))
    else {
        return Vec::new();
    };
    if !otherwise.is_empty() || !first_json.contains(&left_json) {
        return Vec::new();
    }
    let [step] = then.as_slice() else {
        return Vec::new();
    };
    let Some(step) = sentence(step, written) else {
        return Vec::new();
    };
    let twin = Ability::Spell {
        targets: targets.clone(),
        effects: vec![first.clone()],
    };
    written(&twin)
        .iter()
        .filter(|text| text.ends_with('.') && text.contains(", where X is "))
        .map(|text| format!("{text} If X is {least} or more, {step}."))
        .collect()
}

/// "mill five cards. You may put up to two artifact cards from among them into your hand.": cards of a kind chosen among
/// those milled.
fn milled_to_hand(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let [
        mill @ Effect::Mill {
            bind: Some(milled), ..
        },
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(crate::refs::Selector(from)),
            count,
            bind: chosen,
            rest_as: None,
            ..
        },
        Effect::Move {
            what: Ref::Bound(moved),
            to: crate::effect::Destination::Hand,
            ..
        },
    ] = ability.effects()
    else {
        return Vec::new();
    };
    let among = [
        crate::refs::Pred::Is(Ref::Bound(milled.clone())),
        crate::refs::Pred::In(crate::refs::ZoneKind::Graveyard),
    ];
    let kind: Vec<crate::refs::Pred> = from
        .iter()
        .filter(|pred| !among.contains(pred))
        .cloned()
        .collect();
    if moved != chosen || kind.len() + among.len() != from.len() || count.fixed_min() != Some(0) {
        return Vec::new();
    }
    let what = if count.max == Some(crate::value::Amount::Fixed(1)) {
        super::library::what_text(
            &crate::refs::Selector(kind),
            &crate::refs::Bounds::default(),
        )
    } else {
        super::library::what_text(&crate::refs::Selector(kind), count)
    };
    let (Some(what), Some(milled)) = (what, sentence(mill, written)) else {
        return Vec::new();
    };
    after_heads(
        ability,
        written,
        &format!("{milled}. You may put {what} from among them into your hand."),
    )
}

/// "if an Equipment or Spacecraft entered the battlefield under your control this turn": the permanents of a kind that
/// entered under your control this turn, counted as they entered (`Amount::Tally`), from the twin that counts those you
/// still control.
fn entered_this_turn(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    // An intervening "if" of a trigger; an activation restriction words it "if you had ... enter".
    if !matches!(
        ability,
        Ability::Triggered {
            intervening_if: Some(_),
            ..
        }
    ) {
        return Vec::new();
    }
    let Ok(json) = serde_json::to_string(ability) else {
        return Vec::new();
    };
    let opening = r#"{"Tally":{"event":"Entered","who":"EachPlayer","filter":["#;
    let Some(at) = json.find(opening) else {
        return Vec::new();
    };
    let Some((preds, _)) = json[at + opening.len()..].split_once(r#",{"ControlledBy":"You"}]}}"#)
    else {
        return Vec::new();
    };
    // One permanent of a kind, not a token made ("if you created a token this turn").
    let one = format!(r#"{opening}{preds},{{"ControlledBy":"You"}}]}}}},"op":"Ge","right":1}}"#);
    if preds.contains(r#""Token""#) || !json.contains(&one) {
        return Vec::new();
    }
    let tally = format!(r#"{opening}{preds},{{"ControlledBy":"You"}}]}}}}"#);
    let count = format!(r#"{{"Count":[{preds},{{"ControlledBy":"You"}},"EnteredThisTurn"]}}"#);
    let Ok(twin) = serde_json::from_str::<Ability>(&json.replacen(&tally, &count, 1)) else {
        return Vec::new();
    };
    written(&twin)
        .iter()
        .filter_map(|text| {
            let (before, rest) = text.split_once("if you control ")?;
            let (what, after) = rest.split_once(" that entered this turn")?;
            Some([
                format!("{before}if {what} entered the battlefield under your control this turn{after}"),
                format!("{before}if you had {what} enter the battlefield under your control this turn{after}"),
            ])
        })
        .flatten()
        .collect()
}

/// "put it onto the battlefield with seven charge counters on it": a card put onto the battlefield with counters of one
/// kind, from the twin that puts it there tapped.
fn entering_with_counters(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let Some(counters) = take_counters(&mut value) else {
        return Vec::new();
    };
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    let Some(on) = counters_words(&counters) else {
        return Vec::new();
    };
    written(&twin)
        .iter()
        .filter(|text| text.matches(" onto the battlefield tapped").count() == 1)
        .map(|text| {
            text.replacen(
                " onto the battlefield tapped",
                &format!(" onto the battlefield with {on} on it"),
                1,
            )
        })
        .collect()
}

/// Replaces the one entry with counters onto the battlefield in the value by an entry tapped; the counters it had.
fn take_counters(value: &mut serde_json::Value) -> Option<serde_json::Value> {
    if let Some(battlefield) = value
        .get_mut("Battlefield")
        .and_then(serde_json::Value::as_object_mut)
        && battlefield.len() == 1
        && let Some(counters) = battlefield.remove("counters")
    {
        battlefield.insert("tapped".to_owned(), serde_json::Value::Bool(true));
        return Some(counters);
    }
    match value {
        serde_json::Value::Object(fields) => fields.values_mut().find_map(take_counters),
        serde_json::Value::Array(items) => items.iter_mut().find_map(take_counters),
        _ => None,
    }
}

/// "seven charge counters": the one kind of counter and how many.
fn counters_words(counters: &serde_json::Value) -> Option<String> {
    let [pair] = counters.as_array()?.as_slice() else {
        return None;
    };
    let (kind, number): (crate::value::CounterKind, u32) =
        serde_json::from_value(pair.clone()).ok()?;
    super::counters::text(
        &kind,
        &crate::value::Amount::Fixed(i32::try_from(number).ok()?),
    )
}

/// "The owner of up to one target creature draws two cards, then shuffles that creature into their library."
fn owner_draws_then_shuffles(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
) -> Vec<String> {
    use serde_json::json;
    let Ok(serde_json::Value::Array(effects)) = serde_json::to_value(ability.effects()) else {
        return Vec::new();
    };
    let [draw, shuffled] = effects.as_slice() else {
        return Vec::new();
    };
    let Some(cards) = draw
        .get("Draw")
        .filter(|draw| draw.get("who") == Some(&json!({ "OwnerOf": { "Target": 0 } })))
        .and_then(|draw| draw.get("count"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|count| u32::try_from(count).ok())
        .filter(|count| *count > 1)
        .and_then(super::amounts::count_word)
    else {
        return Vec::new();
    };
    let into = json!({ "Move": { "what": { "Target": 0 }, "to": { "Library": { "place": "Shuffled" } } } });
    let targets = ability.targets();
    let creature = targets.first().is_some_and(|spec| {
        spec.what
            == crate::refs::Targetable::Object(crate::refs::Selector(vec![
                crate::refs::Pred::Type(mtg_core::card::CardType::Creature),
            ]))
            && spec.bounds == crate::refs::Bounds::up_to(Some(crate::value::Amount::Fixed(1)))
    });
    if *shuffled != into || targets.len() != 1 || !creature {
        return Vec::new();
    }
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    if let Some(fields) = value
        .as_object_mut()
        .and_then(|map| map.values_mut().next())
        .and_then(serde_json::Value::as_object_mut)
    {
        fields.remove("targets");
    }
    let Ok(untargeted) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    after_heads(
        &untargeted,
        written,
        &format!(
            "the owner of up to one target creature draws {cards} cards, then shuffles that creature into their library."
        ),
    )
}

/// "Whenever you attack, for each opponent you attacked this turn, create a tapped and attacking 2/1 red Klingon Warrior
/// creature token with haste.": a token attacking each of those players, from the twin of one token attacking.
fn attacking_each_attacked(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let [
        Effect::RepeatEach {
            over: Ref::AttackedThisTurn,
            bind,
            effects,
        },
    ] = ability.effects()
    else {
        return Vec::new();
    };
    let [
        Effect::CreateToken {
            against: Some(Ref::Bound(against)),
            attacking: true,
            tapped: true,
            ..
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    if against != bind {
        return Vec::new();
    }
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let Some(create) = value
        .as_object_mut()
        .and_then(|map| map.values_mut().next())
        .and_then(|body| body.get_mut("effects"))
        .and_then(|effects| effects.pointer_mut("/0/RepeatEach/effects/0"))
        .cloned()
    else {
        return Vec::new();
    };
    let mut create = create;
    if let Some(fields) = create
        .get_mut("CreateToken")
        .and_then(serde_json::Value::as_object_mut)
    {
        fields.remove("against");
    }
    if let Some(fields) = value
        .as_object_mut()
        .and_then(|map| map.values_mut().next())
        .and_then(serde_json::Value::as_object_mut)
    {
        fields.insert("effects".to_owned(), serde_json::Value::Array(vec![create]));
    }
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    written(&twin)
        .iter()
        .filter_map(|text| {
            let (head, rest) = text.split_once(", create a ")?;
            let what = rest.strip_suffix(" that's tapped and attacking.")?;
            Some(format!(
                "{head}, for each opponent you attacked this turn, create a tapped and attacking {what}."
            ))
        })
        .collect()
}

/// "reveal cards from the top of your library until you reveal two land cards. Put those land cards onto the battlefield
/// tapped and ...": a reveal that ends at a number of matching cards, from the twin that ends at the first.
fn revealed_until_several(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Some(Effect::RevealUntil {
        filter,
        count: crate::value::Amount::Fixed(count),
        most: None,
        ..
    }) = ability.effects().first()
    else {
        return Vec::new();
    };
    let (Some(number), Some(single), Some(plural)) = (
        u32::try_from(*count)
            .ok()
            .filter(|count| *count > 1)
            .and_then(super::amounts::count_word),
        super::library::filter_text(filter, false),
        super::library::filter_text(filter, true),
    ) else {
        return Vec::new();
    };
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let Some(reveal) = value
        .as_object_mut()
        .and_then(|map| map.values_mut().next())
        .and_then(|body| body.pointer_mut("/effects/0/RevealUntil"))
        .and_then(serde_json::Value::as_object_mut)
    else {
        return Vec::new();
    };
    reveal.remove("count");
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    let one = format!(
        "until you reveal {} {single}. Put that card ",
        super::triggers::article(&single)
    );
    let several = format!("until you reveal {number} {plural}. Put those {plural} ");
    written(&twin)
        .iter()
        .filter(|text| text.matches(&one).count() == 1)
        .map(|text| text.replacen(&one, &several, 1))
        .collect()
}

/// The sentence with its period, which a quoted ability at its end already holds: `... any color."`.
fn stopped(text: &str) -> String {
    if text.ends_with(".\"") {
        text.to_owned()
    } else {
        format!("{text}.")
    }
}

/// "exile the top four cards of your library. Put all permanent cards exiled this way onto the battlefield."
fn permanents_exiled_this_way(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
) -> Vec<String> {
    use serde_json::json;
    let [exile @ Effect::ExileTop { bind, .. }, moved] = ability.effects() else {
        return Vec::new();
    };
    let permanents: Vec<serde_json::Value> = [
        "Artifact",
        "Creature",
        "Enchantment",
        "Land",
        "Planeswalker",
        "Battle",
    ]
    .iter()
    .map(|kind| json!({ "Type": kind }))
    .collect();
    let shape = json!({ "Move": {
        "what": { "Each": [{ "Is": { "Bound": bind } }, { "In": "Exile" }, { "AnyOf": permanents }] },
        "to": { "Battlefield": {} }
    } });
    if serde_json::to_value(moved).ok() != Some(shape) {
        return Vec::new();
    }
    let Some(exiled) = sentence(exile, written) else {
        return Vec::new();
    };
    after_heads(
        ability,
        written,
        &format!("{exiled}. Put all permanent cards exiled this way onto the battlefield."),
    )
}

/// "target opponent exiles cards from the top of their library until they exile a creature card. Assimilate that card.
/// They put the rest on the bottom in a random order.": another player's exile until a card of a kind, then what is done
/// with that card (assimilated, or cast for free), and the rest to the bottom of their library.
fn exiled_until_by_target(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let (
        [
            Effect::ExileUntil {
                who: Ref::Target(0),
                filter,
                total: None,
                r120_count: None,
                bind,
                exiled_as,
            },
            then,
            rest @ ..,
        ],
        [spec],
    ) = (ability.effects(), ability.targets())
    else {
        return Vec::new();
    };
    let found = Ref::Bound(bind.clone());
    let done = match then {
        Effect::R116Assimilate { what } if *what == found => "Assimilate that card.",
        Effect::CastFromZone { .. }
            if serde_json::to_value(then).ok()
                == Some(
                    serde_json::json!({ "CastFromZone": { "what": { "Bound": bind }, "free": true } }),
                ) =>
        {
            "You may cast that card without paying its mana cost."
        }
        _ => return Vec::new(),
    };
    let bottom = serde_json::json!([{ "Move": {
        "what": { "Each": [{ "Is": { "Bound": exiled_as } }, { "In": "Exile" }] },
        "to": { "Library": { "place": "Bottom", "order": "Random" } }
    } }]);
    let rest = match serde_json::to_value(rest).ok() {
        Some(value) if value == bottom => " They put the rest on the bottom in a random order.",
        Some(serde_json::Value::Array(items)) if items.is_empty() => "",
        _ => return Vec::new(),
    };
    let (Some(card), Some(player)) = (
        super::library::filter_text(filter, false),
        chooser_text(&Ref::Target(0), std::slice::from_ref(spec)),
    ) else {
        return Vec::new();
    };
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    if let Some(fields) = value
        .as_object_mut()
        .and_then(|map| map.values_mut().next())
        .and_then(serde_json::Value::as_object_mut)
    {
        fields.remove("targets");
    }
    let Ok(untargeted) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    after_heads(
        &untargeted,
        written,
        &format!(
            "{player} exiles cards from the top of their library until they exile {} {card}. {done}{rest}",
            super::triggers::article(&card)
        ),
    )
}

/// "put a +1/+1 counter on up to one other target creature and two charge counters on up to one target Spacecraft":
/// counters on each of two targets, each part worded as a spell of its own target.
fn counters_on_two_targets(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let ([first, second], [one, two]) = (ability.targets(), ability.effects()) else {
        return Vec::new();
    };
    // Two targets of the same words are told apart by "another", which the pair writers word.
    if first.what == second.what {
        return Vec::new();
    }
    let (
        Effect::PutCounters {
            on: Ref::Target(0), ..
        },
        Effect::PutCounters {
            on: Ref::Target(1), ..
        },
    ) = (one, two)
    else {
        return Vec::new();
    };
    let Ok(mut retargeted) = serde_json::to_value(two) else {
        return Vec::new();
    };
    if let Some(on) = retargeted.pointer_mut("/PutCounters/on") {
        *on = serde_json::json!({ "Target": 0 });
    }
    let Ok(two) = serde_json::from_value::<Effect>(retargeted) else {
        return Vec::new();
    };
    let part = |spec: &crate::refs::TargetSpec, effect: &Effect| {
        let spell = Ability::Spell {
            targets: vec![spec.clone()],
            effects: vec![effect.clone()],
        };
        written(&spell)
            .into_iter()
            .filter_map(|text| {
                text.strip_prefix("Put ")?
                    .strip_suffix('.')
                    .map(str::to_owned)
            })
            .find(|text| !text.contains(". "))
    };
    let (Some(first), Some(second)) = (part(first, one), part(second, &two)) else {
        return Vec::new();
    };
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    if let Some(fields) = value
        .as_object_mut()
        .and_then(|map| map.values_mut().next())
        .and_then(serde_json::Value::as_object_mut)
    {
        fields.remove("targets");
    }
    let Ok(untargeted) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    after_heads(&untargeted, written, &format!("put {first} and {second}."))
}
