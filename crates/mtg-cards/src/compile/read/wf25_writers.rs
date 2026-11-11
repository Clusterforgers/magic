//! wf25 writers for the shapes drafts use most: the object of the event named "it" or "that creature" as the object of a
//! step, mana for each counter removed as the cost, a card that begins the game on the battlefield, a creature that
//! can't attack or block alone, blocks any number of creatures or can't block strong creatures.

use serde_json::Value;

use super::counters;
use crate::ability::Ability;
use crate::modify::{CombatQuirk, GameRule, Modification};
use crate::refs::{Pred, Ref, Selector, TargetSet, TargetSpec, Targetable, ZoneKind};
use crate::value::{Amount, Cmp};

/// The extra blocks the translator gives "can block any number of creatures".
const ANY_NUMBER: u32 = 99;

/// The steps whose object may be the object of the event.
const VERBS: [&str; 8] = [
    "Destroy",
    "Exile",
    "Tap",
    "Untap",
    "Counter",
    "Transform",
    "RemoveFromCombat",
    "Move",
];

/// The nouns "that ..." may name the object of the event with, when the head of the trigger says them.
const NOUNS: [&str; 8] = [
    "creature",
    "land",
    "permanent",
    "planeswalker",
    "artifact",
    "enchantment",
    "spell",
    "card",
];

/// The wordings of an ability no other writer says whole: "If this card is in your opening hand, you may begin the game
/// with it on the battlefield."
pub(super) fn wf25_whole(ability: &Ability) -> Option<Vec<String>> {
    matches!(ability, Ability::OpeningHand).then(|| {
        vec!["If ~ is in your opening hand, you may begin the game with it on the battlefield.".to_owned()]
    })
}

/// The combat rules of the card itself the r7d table leaves out: "~ can't attack alone", "~ can't block alone".
pub(super) fn wf25_combat_text(quirk: &CombatQuirk) -> Option<&'static str> {
    match quirk {
        CombatQuirk::CantAttackAlone => Some("~ can't attack alone"),
        CombatQuirk::CantBlockAlone => Some("~ can't block alone"),
        _ => None,
    }
}

/// "~ can't attack or block alone.": the card's two rules in one sentence.
pub(super) fn wf25_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    let quirk = |ability: &Ability| match ability {
        Ability::Rule {
            rule:
                GameRule::Combat {
                    who: Ref::This,
                    quirk,
                },
            condition: None,
            from: ZoneKind::Battlefield,
        } => Some(quirk.clone()),
        _ => None,
    };
    match (quirk(first), quirk(second)) {
        (Some(CombatQuirk::CantAttackAlone), Some(CombatQuirk::CantBlockAlone)) => {
            vec!["~ can't attack or block alone.".to_owned()]
        }
        _ => Vec::new(),
    }
}

/// The predicate of a static the statics table leaves out: "can block any number of creatures", and "can't block
/// creatures with power 2 or greater" (it can block only what is not a creature or lacks the quality).
pub(super) fn wf25_predicate(mods: &[Modification], conditional: bool) -> Option<String> {
    match mods {
        [Modification::BlocksAdditional(ANY_NUMBER)] if !conditional => {
            Some("can block any number of creatures".to_owned())
        }
        [Modification::CanBlockOnly(Selector(only))] => {
            let [Pred::AnyOf(either)] = only.as_slice() else {
                return None;
            };
            let [Pred::Not(kind), Pred::Not(quality)] = either.as_slice() else {
                return None;
            };
            let creature = Pred::Type(mtg_core::card::CardType::Creature);
            if **kind != creature {
                return None;
            }
            // Only a stat compared is said this way: "with power 2 or greater", "with mana value 3 or less".
            let Pred::Compare { op, .. } = &**quality else {
                return None;
            };
            matches!(op, Cmp::Ge | Cmp::Le).then_some(())?;
            let said = Selector(vec![creature, (**quality).clone()]);
            Some(format!(
                "can't block {}",
                super::selectors::phrase_text(&said, super::selectors::Form::All)?
            ))
        }
        _ => None,
    }
}

/// The ability with every counter of one kind removed as its cost ("Remove any number of storage counters from ~") paid
/// for mana: the twin that adds the mana once, the counter's name, and whether a first mana comes before the counted.
fn removed_twin(ability: &Ability) -> Option<(Ability, String, bool)> {
    let value = serde_json::to_value(ability).ok()?;
    let body = value.get("Activated")?;
    // X is the counters removed and the times the mana is added, nothing else.
    if value.to_string().matches("\"X\"").count() != 2 {
        return None;
    }
    let cost = body.get("cost")?.as_array()?;
    let removed = cost
        .iter()
        .filter_map(|item| item.get("RemoveCounters"))
        .find(|item| item.get("count") == Some(&Value::String("X".to_owned())))?;
    if removed.get("from").is_some() {
        return None;
    }
    let counter = serde_json::from_value(removed.get("counter")?.clone()).ok()?;
    let one = counters::text(&counter, &Amount::one())?;
    let name = one
        .strip_prefix("an ")
        .or_else(|| one.strip_prefix("a "))?
        .to_owned();
    let effects = body.get("effects")?.as_array()?;
    let counted = |effect: &Value| -> Option<Value> {
        let mut add = effect.get("AddMana")?.clone();
        (add.get("times")? == "X").then_some(())?;
        add.as_object_mut()?.remove("times");
        Some(add)
    };
    let (once, first) = match effects.as_slice() {
        [only] => (counted(only)?, false),
        [first, then] => {
            let once = counted(then)?;
            (first.get("AddMana")? == &once).then_some(())?;
            (once, true)
        }
        _ => return None,
    };
    let mut twin = value.clone();
    twin["Activated"]["effects"] = Value::Array(vec![serde_json::json!({ "AddMana": once })]);
    Some((serde_json::from_value(twin).ok()?, name, first))
}

/// "{T}, Remove any number of storage counters from ~: Add {R} for each storage counter removed this way.", "..., then
/// add an additional {B} for each charge counter removed this way."
pub(super) fn wf25_removed_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Some((twin, name, first)) = removed_twin(ability) else {
        return;
    };
    let removed = format!("Remove X {name}s from ~");
    for text in written(&twin) {
        let Some((cost, add)) = text.split_once(": ") else {
            continue;
        };
        let Some(mana) = add
            .strip_prefix("Add ")
            .and_then(|rest| rest.strip_suffix('.'))
        else {
            continue;
        };
        if !cost.contains(&removed) || mana.contains(' ') {
            continue;
        }
        let cost = cost.replacen(&removed, &format!("Remove any number of {name}s from ~"), 1);
        let each = format!("for each {name} removed this way");
        let said = if first {
            format!("{cost}: Add {mana}, then add an additional {mana} {each}.")
        } else {
            format!("{cost}: Add {mana} {each}.")
        };
        if !texts.contains(&said) {
            texts.push(said);
        }
    }
}

/// The trigger with the object of the event as the object of its steps swapped for the card, the verbs swapped, and
/// whether its steps name the card otherwise (then only the swapped verbs' "~" is the object of the event); `None` for
/// any other ability, or when the card is the object of a verb that is swapped too.
fn as_itself(ability: &Ability) -> Option<(Ability, Vec<&'static str>, bool)> {
    let mut value = serde_json::to_value(ability).ok()?;
    let body = value.get_mut("Triggered")?;
    // A limit's sentence is written apart from the head these wordings split off.
    if body.get("limit").is_some()
        || body
            .get("if")
            .is_some_and(|held| held.to_string().contains("\"This\""))
    {
        return None;
    }
    let effects = body.get_mut("effects")?;
    let original = effects.clone();
    let text = original.to_string();
    let mut swapped = Vec::new();
    swap(effects, &mut swapped);
    (!swapped.is_empty()).then_some(())?;
    // "sacrifice ~ and counter that spell": the card acts under verbs of its own only.
    if swapped.iter().any(|verb| acts_on_itself(&original, verb)) {
        return None;
    }
    // A cost or a step of the card's own ("SacrificeSelf", "This", damage it deals) names it too.
    let itself = ["This", "Self", "DealDamage", "Fight"]
        .iter()
        .any(|word| text.contains(word));
    if itself && swapped.iter().any(|verb| phrase(verb).is_none()) {
        return None;
    }
    Some((serde_json::from_value(value).ok()?, swapped, itself))
}

/// Whether a step of the verb acts on the card itself, at any depth.
fn acts_on_itself(value: &Value, verb: &str) -> bool {
    match value {
        Value::Array(items) => items.iter().any(|item| acts_on_itself(item, verb)),
        Value::Object(map) => map.iter().any(|(key, inner)| {
            (key == verb && inner.get("what").is_some_and(|what| *what == "This"))
                || acts_on_itself(inner, verb)
        }),
        _ => false,
    }
}

/// The words before the object of a swapped verb, where the card's other steps name it too.
fn phrase(verb: &str) -> Option<&'static str> {
    Some(match verb {
        "Destroy" => "destroy ",
        "Exile" => "exile ",
        "Tap" => "tap ",
        "Untap" => "untap ",
        "Counter" => "counter ",
        "Transform" => "transform ",
        _ => return None,
    })
}

/// Swaps `"what": "Triggering"` of the verbs among the steps (and the steps of a "you may") for `"This"`, naming each
/// verb swapped. A step under a condition or a payment stays: its sentence order is its own.
fn swap(effects: &mut Value, swapped: &mut Vec<&'static str>) {
    let Some(list) = effects.as_array_mut() else {
        return;
    };
    for effect in list {
        let Some(map) = effect.as_object_mut() else {
            continue;
        };
        for (key, inner) in map.iter_mut() {
            if key == "May"
                && let Some(steps) = inner.get_mut("effects")
            {
                swap(steps, swapped);
            }
            if let Some(verb) = VERBS.iter().find(|verb| **verb == key.as_str())
                && let Some(what) = inner.get_mut("what")
                && *what == "Triggering"
            {
                *what = Value::String("This".to_owned());
                swapped.push(verb);
            }
        }
    }
}

/// Whether the head of a trigger says the noun as a word.
fn says(head: &str, noun: &str) -> bool {
    head.split(|c: char| !c.is_alphanumeric())
        .any(|word| word == noun || word == format!("{noun}s"))
}

/// The pronoun wordings of an effect part written for the card: every "~" is the object of the event.
fn named_in(rest: &str, name: &str) -> String {
    let mut out = String::with_capacity(rest.len());
    let mut first = true;
    let mut pieces = rest.split('~');
    out.push_str(pieces.next().unwrap_or_default());
    for piece in pieces {
        let starts = out.is_empty() || out.ends_with(". ");
        let word = if first { name } else { "it" };
        first = false;
        let (word, piece) = match piece.strip_prefix("'s") {
            Some(after) => (
                if word == "it" {
                    "its".to_owned()
                } else {
                    format!("{word}'s")
                },
                after,
            ),
            None => (word.to_owned(), piece),
        };
        if starts {
            out.push_str(&super::render::capitalize(&word));
        } else {
            out.push_str(&word);
        }
        out.push_str(piece);
    }
    out
}

/// The heads the writers give the trigger, from its twin that only draws a card; without their comma.
fn heads_of(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let Some(body) = value.get_mut("Triggered").and_then(Value::as_object_mut) else {
        return Vec::new();
    };
    body.remove("targets");
    body.insert("effects".to_owned(), serde_json::json!([{ "Draw": {} }]));
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return Vec::new();
    };
    let mut heads: Vec<String> = written(&twin)
        .iter()
        .filter_map(|text| {
            text.strip_suffix(", draw a card.")
                .or_else(|| text.strip_suffix(", you draw a card."))
                .map(str::to_owned)
        })
        .collect();
    heads.dedup();
    heads
}

/// The text split after the head of the trigger (without its comma), when it opens with one of the heads.
fn split_head<'t>(text: &'t str, heads: &[String]) -> Option<(&'t str, &'t str)> {
    heads
        .iter()
        .filter_map(|head| {
            // "When" and "Whenever" are one head (CR 603.1).
            let (_, core) = head.split_once(' ')?;
            let at = text.find(&format!("{core}, "))?;
            (at <= "Whenever ".len()).then(|| text.split_at(at + core.len()))
        })
        .max_by_key(|(head, _)| head.len())
        .and_then(|(head, rest)| Some((head, rest.strip_prefix(", ")?)))
}

/// "Whenever a creature is dealt damage, destroy it.", "When a player casts a spell, counter that spell.", "Whenever
/// this creature deals combat damage to a creature, tap that creature and it doesn't untap during its controller's next
/// untap step.": the object of the event named in its steps. "It" stands only when the head does not name the card.
pub(super) fn wf25_event_object_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Some((itself, swapped, own)) = as_itself(ability) else {
        return;
    };
    let heads = heads_of(ability, written);
    let mut found = Vec::new();
    for text in written(&itself) {
        let Some((head, rest)) = split_head(&text, &heads) else {
            continue;
        };
        // A card exiled from a graveyard is said with its zone, which these wordings leave out.
        if swapped.contains(&"Exile")
            && ["discard", "graveyard", "dies"]
                .iter()
                .any(|word| head.contains(word))
        {
            continue;
        }
        // A wording that already names the card by a pronoun would hide which object it is.
        if !own && rest.matches('~').count() < swapped.len() {
            continue;
        }
        let mut names: Vec<String> = Vec::new();
        // "It" would name the card where the head or its steps name it too.
        if !head.contains('~') && !own {
            names.push("it".to_owned());
        }
        for noun in NOUNS {
            let card = noun == "card"
                && ["dies", "graveyard", "exile"]
                    .iter()
                    .any(|word| head.contains(word));
            if says(head, noun) || card {
                names.push(format!("that {noun}"));
            }
        }
        // "Whenever enchanted creature deals damage to a creature, destroy the other creature."
        if head.contains(" creature deals ") && head.contains(" damage to a creature") {
            names.push("the other creature".to_owned());
        }
        for name in names {
            let said = if own {
                // Each swapped step must say its object where it can be told from the card's own.
                let mut rest = format!(" {rest}");
                let mut whole = true;
                for verb in &swapped {
                    let Some(phrase) = phrase(verb) else {
                        whole = false;
                        break;
                    };
                    let step = format!(" {phrase}~");
                    whole &= rest.matches(&step).count() == 1;
                    rest = rest.replacen(&step, &format!(" {phrase}{name}"), 1);
                }
                if !whole {
                    continue;
                }
                format!("{head},{rest}")
            } else {
                format!("{head}, {}", named_in(rest, &name))
            };
            let mut said = vec![said];
            for rewrite in REWRITES {
                let more: Vec<String> = said.iter().filter_map(|text| rewrite(text)).collect();
                said.extend(more);
            }
            for said in said {
                if !found.contains(&said) && !texts.contains(&said) {
                    found.push(said);
                }
            }
        }
    }
    texts.extend(found);
}

/// The other printed words of a sentence about the object of the event, each tried on every wording before it.
const REWRITES: [fn(&str) -> Option<String>; 7] = [
    // "tap that creature and it doesn't untap during its controller's next untap step"
    |text| {
        let joined = text
            .replace(
                ". That creature doesn't untap during",
                " and it doesn't untap during",
            )
            .replace(". It doesn't untap during", " and it doesn't untap during");
        (joined != text).then_some(joined)
    },
    |text| {
        text.contains(". That creature doesn't untap during").then(|| {
            text.replace(
                ". That creature doesn't untap during",
                ". It doesn't untap during",
            )
        })
    },
    // The object of the event is one object.
    |text| {
        text.contains(". They can't be regenerated")
            .then(|| text.replace(". They can't be regenerated", ". It can't be regenerated"))
    },
    |text| {
        text.contains(". They can't be regenerated").then(|| {
            text.replace(
                ". They can't be regenerated",
                ". The creature can't be regenerated",
            )
        })
    },
    // "Whenever a permanent you control enters tapped": the state it enters in, said after the verb.
    |text| {
        let (head, rest) = text.split_once(" enters, ")?;
        let (opener, object) = head.split_once(" a tapped ")?;
        Some(format!("{opener} a {object} enters tapped, {rest}"))
    },
    // A card put into your graveyard is yours: "put that card on top of your library".
    |text| {
        let (head, rest) = text.split_once(", ")?;
        if !head.contains(" into your graveyard") {
            return None;
        }
        let yours = rest
            .replace(" its owner's library", " your library")
            .replace(" its owner's hand", " your hand");
        (yours != rest).then(|| format!("{head}, {yours}"))
    },
    // "Whenever a land is tapped for mana": whoever taps it.
    |text| {
        text.strip_prefix("Whenever a player taps a land for mana, ")
            .map(|rest| format!("Whenever a land is tapped for mana, {rest}"))
    },
];

/// "up to two target cards from a single graveyard": several cards in graveyards that one player owns, as the plain
/// targets with "a single graveyard"; `None` for any other specification.
pub(super) fn wf25_single_graveyard(
    spec: &TargetSpec,
    written: fn(&TargetSpec) -> Option<String>,
) -> Option<String> {
    let Targetable::Together {
        what,
        rule: TargetSet::SameController,
    } = &spec.what
    else {
        return None;
    };
    let Targetable::Object(selector) = &**what else {
        return None;
    };
    // One graveyard of any player: a graveyard named by its owner is single already.
    if !selector.0.contains(&Pred::In(ZoneKind::Graveyard))
        || selector.0.iter().any(|pred| matches!(pred, Pred::OwnedBy(_)))
        || spec.bounds.is_one()
    {
        return None;
    }
    let plain = TargetSpec {
        what: (**what).clone(),
        ..spec.clone()
    };
    let text = written(&plain)?;
    [" from a graveyard", " from graveyards"]
        .iter()
        .find_map(|graveyards| text.strip_suffix(graveyards))
        .map(|cards| format!("{cards} from a single graveyard"))
}

/// The block rules in their other printed words: "can block any number of creatures" is also printed "can block an
/// additional ninety-nine creatures each combat", and "can't block creatures with power 2 or greater and can't be
/// blocked by creatures with power 2 or greater" is "can't block or be blocked by creatures with power 2 or greater".
pub(super) fn wf25_block_wordings(texts: &mut Vec<String>) {
    let mut more = Vec::new();
    for text in texts.iter() {
        if text.contains("can block any number of creatures") {
            more.push(text.replace(
                "can block any number of creatures",
                "can block an additional ninety-nine creatures each combat",
            ));
        }
        if let Some((before, after)) = text.split_once("can't block ")
            && let Some((blocked, rest)) = after.split_once(" and can't be blocked by ")
            && let Some(tail) = rest.strip_prefix(blocked)
        {
            more.push(format!(
                "{before}can't block or be blocked by {blocked}{tail}"
            ));
        }
    }
    for text in more {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}
