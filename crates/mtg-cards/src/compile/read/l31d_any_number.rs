//! "Sacrifice any number of lands" (`Effect::SacrificeUpTo` of every matching permanent, bound as "sacrificed") and
//! "discard any number of cards" (an optional `Effect::Discard` of the whole hand, bound as "discarded"), with the amounts
//! after them that count what the step moved: "that many", "that much" and "for each permanent sacrificed this way".
//! The reader reads the step from the twin "You sacrifice two lands." and the rest with X for the count; the writer
//! words the twin with the count as the amount of an event ("that many") or as the creatures a devour counts ("for each
//! creature it devoured") and puts the card's words back.

use serde_json::{Value, json};

use crate::ability::Ability;
use crate::compile::classify::ClauseKind;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::Amount;

/// The name the sacrifice binds.
const SACRIFICED: &str = "sacrificed";
/// The name the discard binds.
const DISCARDED: &str = "discarded";
/// The words the writer puts in place of the count before the card's own come back.
const DEVOURED: &str = "creature it devoured";

/// The two steps.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Verb {
    /// "sacrifice any number of ...".
    Sacrifice,
    /// "discard any number of ...".
    Discard,
}

impl Verb {
    /// The verb without its first letter, which a sentence capitalizes.
    const fn stem(self) -> &'static str {
        match self {
            Verb::Sacrifice => "acrifice",
            Verb::Discard => "iscard",
        }
    }

    /// "sacrificed this way".
    const fn this_way(self) -> &'static str {
        match self {
            Verb::Sacrifice => "sacrificed this way",
            Verb::Discard => "discarded this way",
        }
    }

    /// The word every noun of the step is ("permanent", "card").
    const fn noun(self) -> &'static str {
        match self {
            Verb::Sacrifice => "permanent",
            Verb::Discard => "card",
        }
    }
}

/// The step that takes every matching object the player chooses, bound under the verb's name.
fn step(verb: Verb, filter: &Selector) -> Effect {
    match verb {
        Verb::Sacrifice => {
            let mut counted = filter.0.clone();
            if !counted.contains(&Pred::ControlledBy(Ref::You)) {
                counted.push(Pred::ControlledBy(Ref::You));
            }
            Effect::SacrificeUpTo {
                who: Ref::You,
                filter: filter.clone(),
                count: Amount::Count(Selector(counted)),
                bind: Some(SACRIFICED.to_owned()),
            }
        }
        Verb::Discard => {
            let mut counted = vec![Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)];
            counted.extend(filter.0.iter().cloned());
            Effect::Discard {
                who: Ref::You,
                count: Amount::Count(Selector(counted)),
                random: false,
                filter: (!filter.0.is_empty()).then(|| filter.clone()),
                by: None,
                optional: true,
                bind: Some(DISCARDED.to_owned()),
            }
        }
    }
}

/// The count of what the step moved, as the effects after it read it.
fn counted(verb: Verb) -> Amount {
    match verb {
        Verb::Sacrifice => Amount::Bound(format!("{SACRIFICED}~moved")),
        Verb::Discard => Amount::Bound(format!("{DISCARDED}~moved")),
    }
}

/// The step the twin "You sacrifice two lands." stands for: what it takes.
fn stand_in(verb: Verb, filter: &Selector, count: i32) -> Effect {
    match verb {
        Verb::Sacrifice => Effect::Sacrifice {
            who: Ref::You,
            filter: filter.clone(),
            count: Amount::Fixed(count),
            bind: None,
        },
        Verb::Discard => Effect::Discard {
            who: Ref::You,
            count: Amount::Fixed(count),
            random: false,
            filter: (!filter.0.is_empty()).then(|| filter.clone()),
            by: None,
            optional: false,
            bind: None,
        },
    }
}

/// The verb and filter of a step this module reads, if `effect` is one.
fn recognized(effect: &Effect) -> Option<(Verb, Selector)> {
    let verb = match effect {
        Effect::SacrificeUpTo { filter, .. } => (Verb::Sacrifice, filter.clone()),
        Effect::Discard { filter, .. } => (Verb::Discard, filter.clone().unwrap_or_default()),
        _ => return None,
    };
    (step(verb.0, &verb.1) == *effect).then_some(verb)
}

/// Every value in `value` equal to `from` replaced by `to`.
fn replaced(value: &mut Value, from: &Value, to: &Value) {
    if value == from {
        *value = to.clone();
        return;
    }
    match value {
        Value::Array(items) => items.iter_mut().for_each(|item| replaced(item, from, to)),
        Value::Object(map) => map.values_mut().for_each(|item| replaced(item, from, to)),
        _ => {}
    }
}

/// The effect list of the ability, for the kinds that have one.
fn effects_mut(ability: &mut Ability) -> Option<&mut Vec<Effect>> {
    match ability {
        Ability::Spell { effects, .. }
        | Ability::Triggered { effects, .. }
        | Ability::Activated { effects, .. } => Some(effects),
        _ => None,
    }
}

/// The text with its first letter in upper case.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The verb, the text before the step, the noun phrase it takes and the text after it ("" or ". ..." or ", then ...").
fn split(text: &str) -> Option<(Verb, &str, &str, &str)> {
    [Verb::Sacrifice, Verb::Discard]
        .into_iter()
        .find_map(|verb| {
            let marker = format!("{} any number of ", verb.stem());
            let at = text.find(&marker)?;
            let start = at.checked_sub(1)?;
            let head = text.get(..start)?;
            (head.is_empty() || head.ends_with(", ")).then_some(())?;
            let after = text.get(at + marker.len()..)?;
            let end = [
                after.find(". "),
                after.find(", then "),
                after.strip_suffix('.').map(str::len),
            ]
            .into_iter()
            .flatten()
            .min()?;
            Some((verb, head, after.get(..end)?, after.get(end..)?))
        })
}

/// The ability `text` says with "sacrifice any number of ..." or "discard any number of ..." as its first step, read
/// with `read` (text, kind, whether X is free): the step from its twin with a count of two, the rest with X for the count
/// of what the step moved. `None` for any other text.
pub(super) fn l31d_any_number_read(
    text: &str,
    kind: ClauseKind,
    read: impl Fn(&str, ClauseKind, bool) -> Option<Ability>,
) -> Option<Ability> {
    if !matches!(kind, ClauseKind::Spell | ClauseKind::Triggered) || text.contains('X') {
        return None;
    }
    let (verb, head, phrase, tail) = split(text)?;
    let filter = match phrase_read(verb, phrase)
        .or_else(|| phrase_read(verb, &phrase.replace("and/or", "and")))
    {
        Some(filter) => filter,
        None => twin_filter(verb, phrase, &read)?,
    };
    let rest = tail
        .strip_prefix(". ")
        .or_else(|| tail.strip_prefix(", then "))?;
    let rest = each_this_way(
        &rest.replace("that many", "X").replace("that much", "X"),
        verb,
    );
    let rest = if head.is_empty() {
        capitalized(&rest)
    } else {
        format!("{head}{rest}")
    };
    let mut ability = read(&rest, kind, true)?;
    effects_mut(&mut ability)?.insert(0, step(verb, &filter));
    let mut value = serde_json::to_value(&ability).ok()?;
    let count = serde_json::to_value(counted(verb)).ok()?;
    replaced(&mut value, &json!("X"), &count);
    replaced(&mut value, &json!("Devoured"), &count);
    serde_json::from_value(value).ok()
}

/// The words of an ability whose effects hold a step this module reads: the twin with a count of two worded, its
/// count put back as "any number of", and the amounts that count what it moved as "that many", "that much" or "for each
/// ... this way". `None` for any other ability.
pub(super) fn l31d_any_number_variants(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
) -> Option<Vec<String>> {
    let mut twin = ability.clone();
    let effects = effects_mut(&mut twin)?;
    let (at, (verb, filter)) = effects
        .iter()
        .enumerate()
        .find_map(|(at, effect)| Some((at, recognized(effect)?)))?;
    effects[at] = stand_in(verb, &filter, 2);
    let original = serde_json::to_string(ability).ok()?;
    if original.contains("\"Triggering\"") || original.contains("\"Devoured\"") {
        return None;
    }
    let value = serde_json::to_value(&twin).ok()?;
    let count = serde_json::to_value(counted(verb)).ok()?;
    let name = match verb {
        Verb::Sacrifice => SACRIFICED,
        Verb::Discard => DISCARDED,
    };
    let nouns = nouns(verb, &filter, written);
    let phrase = phrase(verb, &filter, written)?;
    let mut texts = Vec::new();
    for (stand, devoured) in [("Triggering", false), ("Devoured", true)] {
        let mut value = value.clone();
        replaced(&mut value, &count, &json!(stand));
        // The name is read by nothing but the count.
        if value.to_string().contains(&format!("\"{name}")) {
            return None;
        }
        let Ok(twin) = serde_json::from_value::<Ability>(value) else {
            continue;
        };
        for text in written(&twin) {
            let two = format!("{} two {phrase}", verb.stem());
            if !text.contains(&two) {
                continue;
            }
            let text = text.replacen(&two, &format!("{} any number of {phrase}", verb.stem()), 1);
            // The player of the step is the one of the ability: "Sacrifice ..." as well as "You sacrifice ...".
            let mut plain = Vec::new();
            for (from, to) in [
                ("You s", "S"),
                (", you s", ", s"),
                ("You d", "D"),
                (", you d", ", d"),
            ] {
                let words = format!("{from}{} any number of ", verb.stem());
                if text.contains(&words) {
                    plain.push(text.replacen(
                        &words,
                        &format!("{to}{} any number of ", verb.stem()),
                        1,
                    ));
                }
            }
            let mut found = Vec::new();
            for text in std::iter::once(text).chain(plain) {
                if !devoured {
                    found.push(text);
                } else if text.contains(DEVOURED) {
                    for noun in &nouns {
                        found.push(text.replace(DEVOURED, &format!("{noun} {}", verb.this_way())));
                    }
                }
            }
            for text in found {
                if let Some(either) = and_or(&text, &phrase)
                    && !texts.contains(&either)
                {
                    texts.push(either);
                }
                if !texts.contains(&text) {
                    texts.push(text);
                }
            }
        }
    }
    Some(texts)
}

/// The noun phrase the writer gives the twin step: "lands" in "Sacrifice two lands.".
fn phrase(verb: Verb, filter: &Selector, written: fn(&Ability) -> Vec<String>) -> Option<String> {
    let alone = Ability::Spell {
        targets: Vec::new(),
        effects: vec![stand_in(verb, filter, 2)],
    };
    let marker = format!("{} two ", verb.stem());
    written(&alone).into_iter().find_map(|text| {
        let at = text.find(&marker)?;
        let rest = text.get(at + marker.len()..)?;
        Some(rest.strip_suffix('.')?.to_owned())
    })
}

/// The nouns "for each ... this way" may name: the word for every object of the step, and one of the phrase.
fn nouns(verb: Verb, filter: &Selector, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let mut nouns = vec![verb.noun().to_owned()];
    let alone = Ability::Spell {
        targets: Vec::new(),
        effects: vec![stand_in(verb, filter, 1)],
    };
    for text in written(&alone) {
        for article in ["a ", "an "] {
            let marker = format!("{} {article}", verb.stem());
            if let Some(noun) = text
                .find(&marker)
                .and_then(|at| text.get(at + marker.len()..))
                .and_then(|rest| rest.strip_suffix('.'))
                && !nouns.iter().any(|known| known == noun)
            {
                nouns.push(noun.to_owned());
            }
        }
    }
    nouns
}

/// "artifacts, creatures, and/or lands": the phrase of a list of kinds with "and/or" for its last "and".
fn and_or(text: &str, phrase: &str) -> Option<String> {
    let at = phrase.rfind(" and ")?;
    let either = format!("{} and/or {}", &phrase[..at], &phrase[at + 5..]);
    Some(text.replacen(
        &format!("any number of {phrase}"),
        &format!("any number of {either}"),
        1,
    ))
}

/// The objects the plural noun phrase names ("lands", "other permanents", "artifacts, enchantments, and/or tokens"), read
/// whole; the discard reads the cards of the hand ("creature cards").
fn phrase_read(verb: Verb, words: &str) -> Option<Selector> {
    let tokens = super::tokens::tokenize(words);
    let mut r = super::tokens::Reader::new(&tokens, false);
    let filter = super::selectors::phrase(&mut r, super::selectors::Form::All)?;
    r.end().then_some(())?;
    (verb == Verb::Sacrifice).then_some(filter)
}

/// The text with every "for each <noun> sacrificed this way" as "for each creature it devoured", which the readers of
/// counted amounts read; the writer checks the noun.
fn each_this_way(text: &str, verb: Verb) -> String {
    let mut text = text.to_owned();
    let tail = format!(" {}", verb.this_way());
    while let Some(end) = text.find(&tail) {
        let Some(start) = text[..end].rfind("for each ") else {
            break;
        };
        text.replace_range(start..end + tail.len(), &format!("for each {DEVOURED}"));
    }
    text
}

/// The objects the twin "You sacrifice two lands." or "Discard two creature cards." takes, read by `read`.
fn twin_filter(
    verb: Verb,
    phrase: &str,
    read: &impl Fn(&str, ClauseKind, bool) -> Option<Ability>,
) -> Option<Selector> {
    let twin = match verb {
        Verb::Sacrifice => format!("You sacrifice two {}.", phrase.replace("and/or", "and")),
        Verb::Discard => format!("Discard two {phrase}."),
    };
    let Ability::Spell { targets, effects } = read(&twin, ClauseKind::Spell, false)? else {
        return None;
    };
    targets.is_empty().then_some(())?;
    match effects.as_slice() {
        [
            Effect::Sacrifice {
                who: Ref::You,
                filter,
                count: Amount::Fixed(2),
                bind: None,
            },
        ] if verb == Verb::Sacrifice => Some(filter.clone()),
        [
            Effect::Discard {
                who: Ref::You,
                count: Amount::Fixed(2),
                random: false,
                filter,
                by: None,
                optional: false,
                bind: None,
            },
        ] if verb == Verb::Discard => Some(filter.clone().unwrap_or_default()),
        _ => None,
    }
}
