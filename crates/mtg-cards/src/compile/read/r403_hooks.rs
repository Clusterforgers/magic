//! r40-3 wordings read from a twin another reader reads and written from that twin's wordings: "tap enchanted creature
//! and remove it from combat" (the twin names the creature again in a sentence of its own) and "As long as enchanted
//! creature isn't legendary, it doesn't untap ..." (the twin names the creature again after the condition); "enters with
//! your choice of a +1/+1, first strike, or vigilance counter on it" (the twin names "counter" with each kind), "enters with
//! two +1/+1 counters and a trample counter on it" (the twin names the first kind alone) and "put the same number of each
//! kind of counter on that creature" (the twin draws a card).

use super::ClauseKind;
use super::tokens::{Reader, tokenize};
use crate::ability::Ability;
use crate::did::Action;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::trigger::{MoreCounters, ReplaceBy, TriggerPattern};
use crate::value::{Amount, CounterKind};

/// The step a trigger that copies the counters of the card is written with.
const R403_SAME_COUNTERS: &str = "put the same number of each kind of counter on that creature";
/// The condition of a trigger that the card has a counter of any kind, as the trigger writes it after its head.
const R403_HAS_COUNTERS: &str = ", if ~ has counters on it, ";
/// The step its twin is written with.
const R403_DRAW: &str = "draw a card";

/// The nouns an Aura or Equipment calls the permanent it is attached to.
const R403_ATTACHED: [&str; 3] = [
    "enchanted creature",
    "equipped creature",
    "enchanted permanent",
];

/// The joined form "tap N and remove it from combat" of the twin "tap N. Remove N from combat".
fn r403_combat_pairs() -> impl Iterator<Item = (String, String)> {
    R403_ATTACHED.iter().map(|noun| {
        (
            format!("tap {noun} and remove it from combat"),
            format!("tap {noun}. Remove {noun} from combat"),
        )
    })
}

/// The twin of "As long as N ..., it S" with N again in place of "it", when the condition names the attached permanent.
fn r403_it_twin(text: &str) -> Option<String> {
    let rest = text.strip_prefix("As long as ")?;
    let noun = R403_ATTACHED.iter().find(|noun| rest.starts_with(**noun))?;
    let (condition, body) = rest.split_once(", it ")?;
    Some(format!("As long as {condition}, {noun} {body}"))
}

/// The ability a twin of `text` reads as, for the wordings of this pack; `read` reads a clause of the same kind.
pub(super) fn r403_read(
    text: &str,
    kind: ClauseKind,
    read: impl Fn(&str) -> Option<Ability>,
) -> Option<Ability> {
    if kind == ClauseKind::Triggered
        && let Some(found) = super::r403_excess::r403_excess_read(text, &read)
    {
        return Some(found);
    }
    if let Some(found) = r403_shared_read(text, &read) {
        return Some(found);
    }
    if kind == ClauseKind::Triggered
        && let Some(found) = r403_upkeep_read(text, &read)
    {
        return Some(found);
    }
    if let Some(twin) = r403_while_twin(text) {
        return read(&twin).filter(|found| matches!(found, Ability::Replacement { .. }));
    }
    if kind == ClauseKind::Triggered
        && let Some((joined, twin)) =
            r403_combat_pairs().find(|(joined, _)| text.contains(joined.as_str()))
    {
        return read(&text.replacen(&joined, &twin, 1));
    }
    if kind == ClauseKind::Triggered
        && let Some((head, body)) = text.split_once(R403_HAS_COUNTERS)
    {
        let mut ability = read(&format!("{head}, {body}"))?;
        let Ability::Triggered { intervening_if, .. } = &mut ability else {
            return None;
        };
        intervening_if.is_none().then_some(())?;
        *intervening_if = Some(r403_has_counters());
        return Some(ability);
    }
    if kind == ClauseKind::Triggered
        && let Some(head) = text.strip_suffix(&format!("{R403_SAME_COUNTERS}."))
    {
        let mut ability = read(&format!("{head}{R403_DRAW}."))?;
        let Ability::Triggered { effects, .. } = &mut ability else {
            return None;
        };
        (effects.as_slice() == [r403_draw()]).then_some(())?;
        *effects = vec![r403_same_counters()];
        return Some(ability);
    }
    if kind == ClauseKind::Static
        && let Some(twin) = r403_it_twin(text)
    {
        return read(&twin);
    }
    if let Some(found) = r403_different_read(text) {
        return Some(found);
    }
    if let Some(twin) = r403_choice_twin(text) {
        return read(&twin);
    }
    r403_also_read(text, &read)
}

/// The card has one or more counters of any kind.
fn r403_has_counters() -> crate::value::Condition {
    crate::value::Condition::Compare {
        left: Amount::AnyCounters(Ref::This),
        op: crate::value::Cmp::Ge,
        right: Amount::Fixed(1),
    }
}

/// "As long as C, if X, R" as the twin "If X while C, R".
fn r403_while_twin(text: &str) -> Option<String> {
    let rest = text.strip_prefix("As long as ")?;
    let (condition, rest) = rest.split_once(", if ")?;
    let (event, result) = rest.split_once(", ")?;
    Some(format!("If {event} while {condition}, {result}"))
}

/// "If X while C, R" as "As long as C, if X, R".
fn r403_as_long_as(text: &str) -> Option<String> {
    let rest = text.strip_prefix("If ")?;
    let (head, result) = rest.split_once(", ")?;
    let (event, condition) = head.split_once(" while ")?;
    Some(format!("As long as {condition}, if {event}, {result}"))
}

/// The heads of a trigger on an upkeep cost of the card paid or not, with the action each logs and who pays.
const R403_UPKEEP_HEADS: [(&str, Action, Ref); 3] = [
    (
        "Whenever you pay ~'s cumulative upkeep",
        Action::PaidCumulativeUpkeep,
        Ref::You,
    ),
    (
        "When a player doesn't pay ~'s cumulative upkeep",
        Action::DeclinedCumulativeUpkeep,
        Ref::EachPlayer,
    ),
    (
        "When ~'s echo cost is paid",
        Action::PaidEcho,
        Ref::EachPlayer,
    ),
];
/// The head of the twin a trigger on an upkeep cost is read and written with.
const R403_TWIN_HEAD: &str = "Whenever you investigate";
/// The step of a trigger on a player who didn't pay: they exile their library.
const R403_EXILES_LIBRARY: &str = "that player exiles all cards from their library.";

/// The words "this <type>'s" of a card naming itself, as "~'s".
fn r403_own(text: &str) -> String {
    let mut own = text.to_owned();
    for noun in [
        "this enchantment's",
        "this creature's",
        "this artifact's",
        "this permanent's",
    ] {
        own = own.replace(noun, "~'s");
    }
    own
}

/// Every card in the library of the player a trigger is about.
fn r403_their_library() -> Ref {
    Ref::Each(crate::refs::Selector(vec![
        crate::refs::Pred::In(crate::refs::ZoneKind::Library),
        crate::refs::Pred::OwnedBy(Ref::TriggeringPlayer),
    ]))
}

/// A trigger on an upkeep cost of the card paid or not, read from the twin "Whenever you investigate, S." or, for "that
/// player exiles all cards from their library", read whole.
fn r403_upkeep_read(text: &str, read: &impl Fn(&str) -> Option<Ability>) -> Option<Ability> {
    let own = r403_own(text);
    let (head, action, who) = R403_UPKEEP_HEADS
        .iter()
        .find(|(head, ..)| own.starts_with(&format!("{head}, ")))?;
    let body = &own[head.len() + 2..];
    let when = TriggerPattern::Did {
        action: *action,
        who: who.clone(),
        what: crate::refs::Selector::this(),
    };
    if body == R403_EXILES_LIBRARY {
        return Some(Ability::Triggered {
            when,
            intervening_if: None,
            limit: None,
            from: crate::refs::ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::Exile {
                what: r403_their_library(),
                bind: None,
                duration: None,
            }],
        });
    }
    let mut ability = read(&format!("{R403_TWIN_HEAD}, {body}"))?;
    let Ability::Triggered {
        when: read_when, ..
    } = &mut ability
    else {
        return None;
    };
    *read_when = when;
    Some(ability)
}

/// The wordings of a trigger on an upkeep cost of the card paid or not.
fn r403_upkeep_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    more: &mut Vec<String>,
) {
    let Ability::Triggered {
        when: TriggerPattern::Did { action, who, what },
        effects,
        ..
    } = ability
    else {
        return;
    };
    let Some((head, ..)) = R403_UPKEEP_HEADS
        .iter()
        .find(|(_, act, by)| act == action && by == who)
    else {
        return;
    };
    if *what != crate::refs::Selector::this() {
        return;
    }
    let exile = Effect::Exile {
        what: r403_their_library(),
        bind: None,
        duration: None,
    };
    if effects.as_slice() == [exile] {
        more.push(format!("{head}, {R403_EXILES_LIBRARY}"));
        return;
    }
    let mut twin = ability.clone();
    if let Ability::Triggered { when, .. } = &mut twin {
        *when = TriggerPattern::Did {
            action: Action::Investigated,
            who: Ref::You,
            what: crate::refs::Selector::default(),
        };
    }
    for text in written(&twin) {
        if let Some(body) = text.strip_prefix(&format!("{R403_TWIN_HEAD}, ")) {
            more.push(format!("{head}, {body}"));
        }
    }
}

/// The permanent types (CR 110.4), in the order of the comprehensive rules.
const R403_PERMANENT_TYPES: [mtg_core::card::CardType; 6] = [
    mtg_core::card::CardType::Artifact,
    mtg_core::card::CardType::Battle,
    mtg_core::card::CardType::Creature,
    mtg_core::card::CardType::Enchantment,
    mtg_core::card::CardType::Land,
    mtg_core::card::CardType::Planeswalker,
];
/// The wordings of a shared type of some types, and the wording of the twin that shares any card type.
const R403_SHARED: [(&str, &str); 3] = [
    ("share a permanent type", "share a card type"),
    ("shares a permanent type", "shares a card type"),
    (
        "shares one of those types with it",
        "shares a card type with it",
    ),
];

/// The ability with each `SharesType` of its JSON made a `SharesTypeOf` of these types, or the reverse.
fn r403_swap_shared(
    ability: &Ability,
    types: Option<&[mtg_core::card::CardType]>,
) -> Option<Ability> {
    fn walk(value: &mut serde_json::Value, types: Option<&serde_json::Value>, changed: &mut bool) {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(types) = types
                    && let Some(with) = map.remove("SharesType")
                {
                    map.insert(
                        "SharesTypeOf".to_owned(),
                        serde_json::json!({ "with": with, "types": types }),
                    );
                    *changed = true;
                } else if types.is_none()
                    && let Some(of) = map.remove("SharesTypeOf")
                {
                    map.insert(
                        "SharesType".to_owned(),
                        of.get("with").cloned().unwrap_or_default(),
                    );
                    *changed = true;
                }
                map.values_mut()
                    .for_each(|inner| walk(inner, types, changed));
            }
            serde_json::Value::Array(items) => items
                .iter_mut()
                .for_each(|inner| walk(inner, types, changed)),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(ability).ok()?;
    let types = types.map(serde_json::to_value).transpose().ok()?;
    let mut changed = false;
    walk(&mut value, types.as_ref(), &mut changed);
    changed.then_some(())?;
    serde_json::from_value(value).ok()
}

/// The card types the first target of a spell names as alternatives ("target artifact or creature").
fn r403_first_types(ability: &Ability) -> Option<Vec<mtg_core::card::CardType>> {
    let Ability::Spell { targets, .. } = ability else {
        return None;
    };
    let crate::refs::Targetable::Object(selector) = &targets.first()?.what else {
        return None;
    };
    let [crate::refs::Pred::AnyOf(any)] = selector.0.as_slice() else {
        return None;
    };
    any.iter()
        .map(|pred| match pred {
            crate::refs::Pred::Type(card_type) => Some(*card_type),
            _ => None,
        })
        .collect()
}

/// "that share a permanent type", "that shares one of those types with it" read from the twin that shares a card type.
fn r403_shared_read(text: &str, read: &impl Fn(&str) -> Option<Ability>) -> Option<Ability> {
    let (words, twin) = R403_SHARED
        .iter()
        .find(|(words, _)| text.contains(*words))?;
    let found = read(&text.replacen(words, twin, 1))?;
    let types = if words.contains("those") {
        r403_first_types(&found)?
    } else {
        R403_PERMANENT_TYPES.to_vec()
    };
    r403_swap_shared(&found, Some(&types))
}

/// The wordings of a spell whose target shares some types with another, from the twin that shares any card type.
fn r403_shared_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    more: &mut Vec<String>,
) {
    let Some(twin) = r403_swap_shared(ability, None) else {
        return;
    };
    let Some(back) = r403_swap_shared(&twin, Some(&R403_PERMANENT_TYPES)) else {
        return;
    };
    let permanent = back == *ability;
    let those = r403_first_types(&twin)
        .and_then(|types| r403_swap_shared(&twin, Some(&types)))
        .is_some_and(|back| back == *ability);
    for text in written(&twin) {
        for (words, twin_words) in R403_SHARED {
            if (words.contains("those") && those || !words.contains("those") && permanent)
                && text.contains(twin_words)
            {
                more.push(text.replacen(twin_words, words, 1));
            }
        }
    }
}

/// A draw of one card by the controller.
fn r403_draw() -> Effect {
    Effect::Draw {
        who: Ref::You,
        count: Amount::Fixed(1),
        bind: None,
    }
}

/// The counters of the card put on the creature the trigger is about, as many of each kind.
fn r403_same_counters() -> Effect {
    Effect::PutCountersOf {
        on: Ref::Triggering,
        of: Ref::This,
        each: None,
        one_kind: false,
        per_kind: None,
    }
}

/// "your choice of a A, B, or C counter" as "your choice of a A counter, a B counter, or a C counter".
fn r403_choice_twin(text: &str) -> Option<String> {
    let (head, rest) = text.split_once("your choice of a ")?;
    let (list, tail) = rest.split_once(" counter on ")?;
    let (leading, final_kind) = list.rsplit_once(", or ")?;
    let mut kinds: Vec<&str> = leading.split(", ").collect();
    kinds.push(final_kind);
    (kinds.len() >= 3 && !kinds.iter().any(|kind| kind.contains("counter"))).then_some(())?;
    let named: Vec<String> = kinds
        .iter()
        .map(|kind| format!("{} {kind} counter", r403_article(kind)))
        .collect();
    let (all_but, end) = named.split_at(named.len() - 1);
    Some(format!(
        "{head}your choice of {}, or {} on {tail}",
        all_but.join(", "),
        end[0]
    ))
}

/// The words of a choice of different counters, around its count and its kinds.
const R403_DIFFERENT: (&str, &str, &str) = (
    "~ enters with your choice of ",
    " different counters on it from among ",
    ".",
);

/// "~ enters with your choice of two different counters on it from among menace, deathtouch, and lifelink."
fn r403_different_read(text: &str) -> Option<Ability> {
    let (head, middle, end) = R403_DIFFERENT;
    let rest = text.strip_prefix(head)?;
    let (number, rest) = rest.split_once(middle)?;
    let list = rest.strip_suffix(end)?;
    let count = super::amounts::number_word(number)?;
    let (leading, final_kind) = list.rsplit_once(", and ")?;
    let mut kinds = Vec::new();
    for name in leading.split(", ").chain(std::iter::once(final_kind)) {
        let tokens = tokenize(name);
        let mut r = Reader::new(&tokens, false);
        kinds.push(super::counters::kind(&mut r)?);
        r.end().then_some(())?;
    }
    Some(Ability::Replacement {
        when: crate::trigger::ReplaceWhen::Enters {
            what: crate::refs::Selector::this(),
        },
        by: ReplaceBy::WithCountersOf { kinds, count },
        condition: None,
        from: crate::refs::ZoneKind::Battlefield,
    })
}

/// The wording `r403_different_read` reads.
fn r403_different_text(kinds: &[CounterKind], count: u32) -> Option<String> {
    let (head, middle, end) = R403_DIFFERENT;
    let names: Option<Vec<&str>> = kinds.iter().map(super::counters::kind_text).collect();
    let names = names?;
    let (last, firsts) = names.split_last()?;
    let number = super::amounts::count_word(count)?;
    Some(format!(
        "{head}{number}{middle}{}, and {last}{end}",
        firsts.join(", ")
    ))
}

/// The article before a word.
fn r403_article(word: &str) -> &'static str {
    if word.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    }
}

/// "your choice of a A counter, a B counter, or a C counter" as "your choice of a A, B, or C counter".
fn r403_choice_joined(text: &str) -> Option<String> {
    let (head, rest) = text.split_once("your choice of ")?;
    let (list, tail) = rest.split_once(" counter on ")?;
    let (leading, final_kind) = list.rsplit_once(" counter, or ")?;
    let mut kinds: Vec<&str> = leading.split(" counter, ").collect();
    kinds.push(final_kind);
    (kinds.len() >= 3).then_some(())?;
    let bare: Option<Vec<&str>> = kinds
        .iter()
        .map(|named| named.split_once(' ').map(|(_, kind)| kind))
        .collect();
    let bare = bare?;
    let (all_but, end) = bare.split_at(bare.len() - 1);
    Some(format!(
        "{head}your choice of {} {}, or {} counter on {tail}",
        r403_article(bare[0]),
        all_but.join(", "),
        end[0]
    ))
}

/// "enters with A and B on it" read as the twin "enters with A on it" with the counters B besides.
fn r403_also_read(text: &str, read: &impl Fn(&str) -> Option<Ability>) -> Option<Ability> {
    let (head, rest) = text.split_once(" enters with ")?;
    let (counted, tail) = rest.split_once(" on it")?;
    let (first, more) = counted.split_once(" and ")?;
    let tokens = tokenize(more);
    let mut r = Reader::new(&tokens, false);
    let (kind, number) = super::counters::entering(&mut r)?;
    r.end().then_some(())?;
    let mut ability = read(&format!("{head} enters with {first} on it{tail}"))?;
    let Ability::Replacement {
        by: ReplaceBy::WithCountersAnd { also, .. },
        ..
    } = &mut ability
    else {
        return None;
    };
    also.push(MoreCounters {
        counter: kind,
        count: number,
    });
    Some(ability)
}

/// Whether the ability taps the attached permanent and then removes it from combat.
fn r403_taps_then_removes(ability: &Ability) -> bool {
    ability.effects().windows(2).any(|pair| {
        matches!(
            pair,
            [
                Effect::Tap {
                    what: Ref::Attached,
                    bind: None
                },
                Effect::RemoveFromCombat {
                    what: Ref::Attached
                }
            ]
        )
    })
}

/// The wordings of this pack added to `texts`, the wordings of `ability` so far.
pub(super) fn r403_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let mut more = Vec::new();
    if r403_taps_then_removes(ability) {
        for text in texts.iter() {
            for (joined, twin) in r403_combat_pairs() {
                if text.contains(twin.as_str()) {
                    more.push(text.replacen(&twin, &joined, 1));
                }
            }
        }
    }
    if let Ability::Static {
        affected: Ref::Attached,
        condition:
            Some(crate::value::Condition::Matches {
                subject: Ref::Attached,
                ..
            }),
        ..
    } = ability
    {
        for text in texts.iter() {
            let Some(rest) = text.strip_prefix("As long as ") else {
                continue;
            };
            let Some(noun) = R403_ATTACHED.iter().find(|noun| rest.starts_with(**noun)) else {
                continue;
            };
            if let Some((condition, body)) = rest.split_once(&format!(", {noun} ")) {
                more.push(format!("As long as {condition}, it {body}"));
            }
        }
    }
    if let Ability::Replacement {
        condition: Some(_), ..
    } = ability
    {
        more.extend(texts.iter().filter_map(|text| r403_as_long_as(text)));
    }
    r403_upkeep_wordings(ability, written, &mut more);
    r403_shared_wordings(ability, written, &mut more);
    super::r403_excess::r403_excess_wordings(ability, written, &mut more);
    r403_counter_wordings(ability, written, &mut more, texts);
    texts.extend(more);
}

/// The wordings of a choice of three or more kinds of counters and of an entry with counters of two kinds, and of a
/// trigger that copies the counters of the card.
fn r403_counter_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    more: &mut Vec<String>,
    texts: &[String],
) {
    if let Ability::Replacement {
        when: crate::trigger::ReplaceWhen::Enters { what },
        by: ReplaceBy::WithCountersOf { kinds, count },
        condition: None,
        from: crate::refs::ZoneKind::Battlefield,
    } = ability
        && *what == crate::refs::Selector::this()
    {
        more.extend(r403_different_text(kinds, *count));
    }
    if let Ability::Replacement {
        by: ReplaceBy::WithCounterOf(kinds),
        ..
    } = ability
        && kinds.len() >= 3
    {
        more.extend(texts.iter().filter_map(|text| r403_choice_joined(text)));
    }
    if let Ability::Replacement {
        by: ReplaceBy::WithCountersAnd { also, .. },
        ..
    } = ability
        && let [MoreCounters { counter, count }] = also.as_slice()
        && let Some(words) = super::counters::entering_text(counter, count)
    {
        let mut twin = ability.clone();
        if let Ability::Replacement {
            by: ReplaceBy::WithCountersAnd { also, .. },
            ..
        } = &mut twin
        {
            also.clear();
        }
        for text in written(&twin) {
            let Some((head, rest)) = text.split_once(" enters with ") else {
                continue;
            };
            if let Some((first, tail)) = rest.split_once(" on it") {
                more.push(format!(
                    "{head} enters with {first} and {words} on it{tail}"
                ));
            }
        }
    }
    if let Ability::Triggered {
        intervening_if: Some(condition),
        ..
    } = ability
        && *condition == r403_has_counters()
    {
        let mut twin = ability.clone();
        if let Ability::Triggered { intervening_if, .. } = &mut twin {
            *intervening_if = None;
        }
        for text in written(&twin) {
            if let Some((head, body)) = text.split_once(", ") {
                more.push(format!("{head}{R403_HAS_COUNTERS}{body}"));
            }
        }
    }
    if let Ability::Triggered { effects, .. } = ability
        && effects.as_slice() == [r403_same_counters()]
    {
        let mut twin = ability.clone();
        if let Ability::Triggered { effects, .. } = &mut twin {
            *effects = vec![r403_draw()];
        }
        for text in written(&twin) {
            if let Some(head) = text.strip_suffix(&format!("{R403_DRAW}.")) {
                more.push(format!("{head}{R403_SAME_COUNTERS}."));
            }
        }
    }
}
