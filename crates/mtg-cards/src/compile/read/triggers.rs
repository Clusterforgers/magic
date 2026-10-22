//! Triggers: the event a triggered ability waits for, and the condition of an intervening "if",
//! read from "when ~ enters", "whenever you cast a creature spell" or "at the beginning of your
//! upkeep" and written back the same way.

use super::amounts::{count_word, number_word};
use super::counters;
use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::{Step, TriggerPattern};
use crate::value::{Amount, Cmp, Condition, CounterKind};

/// Whether the pattern is "when you cycle this card", the one trigger that works from the graveyard.
pub fn cycled_itself(pattern: &TriggerPattern) -> bool {
    matches!(
        pattern,
        TriggerPattern::Cycled { what, by: Ref::You } if what.0 == [Pred::This]
    )
}

/// Whether the pattern is the card itself becoming a target (counted or not), so the card's effects may call it "it".
pub fn targeted_itself(pattern: &TriggerPattern) -> bool {
    match pattern {
        TriggerPattern::Nth { event, .. } => targeted_itself(event),
        TriggerPattern::BecomesTarget { what, .. } => what.0 == [Pred::This],
        _ => false,
    }
}

/// The steps a trigger names and the words for each.
const STEPS: [(Step, &str); 5] = [
    (Step::Upkeep, "upkeep"),
    (Step::Draw, "draw step"),
    (Step::End, "end step"),
    (Step::Main1, "precombat main phase"),
    (Step::Main2, "postcombat main phase"),
];

/// The newer wordings of the main phases: the card text says "first" and "second" where the rules
/// said "precombat" and "postcombat". Both are read; the older is written.
const NEWER_STEPS: [(Step, &str); 2] = [
    (Step::Main1, "first main phase"),
    (Step::Main2, "second main phase"),
];

/// The event of a trigger, from its first word: "When ~ enters", "Whenever you cast a spell",
/// "At the beginning of your upkeep".
pub fn pattern(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    if r.words("at the beginning of") {
        return step(r);
    }
    if !(r.word("when") || r.word("whenever")) {
        return None;
    }
    if let Some(found) = r.attempt(player_event) {
        return Some(found);
    }
    if let Some(found) = r.attempt(counter_event) {
        return Some(found);
    }
    if let Some(found) = r.attempt(together) {
        return Some(found);
    }
    let what = subject(r)?;
    verbs(r, &what)
}

/// The events that happen together: "one or more creatures you control deal combat damage to a player", "one or more
/// cards are put into your graveyard from your library", "one or more creatures attack".
fn together(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    let at_least = if r.words("one or more") {
        1
    } else {
        let number = number_word(r.take_word()?)?;
        r.need("or more")?;
        number
    };
    if at_least == 1 && r.words("cards are put into your graveyard from your library") {
        let what = Selector(vec![Pred::OwnedBy(Ref::You)]);
        return Some(TriggerPattern::Batch(Box::new(
            TriggerPattern::ZoneChange {
                what,
                from: Some(ZoneKind::Library),
                to: Some(ZoneKind::Graveyard),
            },
        )));
    }
    let what = phrase(r, Form::All)?;
    if r.word("attack") {
        return Some(TriggerPattern::GroupAttacks { what, at_least });
    }
    if at_least != 1 || !r.word("deal") {
        return None;
    }
    let damage = damage_dealt(r, &what)?;
    Some(TriggerPattern::Batch(Box::new(damage)))
}

/// "a +1/+1 counter is put on ~", "a counter is removed from a creature you control".
fn counter_event(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    (r.word("a") || r.word("an")).then_some(())?;
    // A plain "counter" is a counter of any kind.
    let counter = if r.word("counter") {
        None
    } else {
        Some(counters::trigger_kind(r)?)
    };
    let putting = if r.words("is put on") {
        true
    } else if r.words("is removed from") {
        false
    } else {
        return None;
    };
    let what = subject(r)?;
    Some(if putting {
        TriggerPattern::CountersPut { what, counter }
    } else {
        TriggerPattern::CountersRemoved { what, counter }
    })
}

/// "your upkeep", "each end step", "each opponent's draw step", "combat on your turn", "each combat".
fn step(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    let combat = |whose| TriggerPattern::StepBegins {
        step: Step::BeginCombat,
        whose,
    };
    if r.words("combat on your turn") {
        return Some(combat(Ref::You));
    }
    if r.words("each combat") {
        return Some(combat(Ref::EachPlayer));
    }
    let whose = if r.word("your") {
        Ref::You
    } else if r.words("each opponent's") {
        Ref::Opponents
    } else if r.word("each") {
        Ref::EachPlayer
    } else {
        return None;
    };
    let (step, _) = STEPS
        .iter()
        .chain(&NEWER_STEPS)
        .find(|(_, words)| r.words(words))?;
    Some(TriggerPattern::StepBegins { step: *step, whose })
}

/// The one doing it: "you", "an opponent" or "a player", and whether the verb takes an "s".
fn player(r: &mut Reader<'_>) -> Option<(Ref, bool)> {
    if r.word("you") {
        Some((Ref::You, false))
    } else if r.words("an opponent") {
        Some((Ref::Opponents, true))
    } else if r.words("a player") {
        Some((Ref::EachPlayer, true))
    } else {
        None
    }
}

/// "you cast a creature spell", "an opponent casts a spell", "you sacrifice a Clue".
fn player_event(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    let (by, third) = player(r)?;
    let verb = r.take_word()?;
    if verb == if third { "casts" } else { "cast" } {
        if let Some(nth) = each_turn(r, "spell") {
            let event = TriggerPattern::SpellCast {
                what: Selector::default(),
                by,
            };
            return Some(TriggerPattern::Nth {
                event: Box::new(event),
                nth,
            });
        }
        let mut spell = subject(r)?.0;
        // The pattern is about a spell already; the selector keeps what the spell must be.
        if spell.first() != Some(&Pred::Spell) || spell.contains(&Pred::Other) {
            return None;
        }
        spell.remove(0);
        Some(TriggerPattern::SpellCast {
            what: Selector(spell),
            by,
        })
    } else if verb == if third { "sacrifices" } else { "sacrifice" } {
        Some(TriggerPattern::Sacrificed {
            what: subject(r)?,
            by,
        })
    } else if verb == if third { "gains" } else { "gain" } {
        r.need("life")?;
        Some(TriggerPattern::LifeGained { who: by })
    } else if verb == if third { "loses" } else { "lose" } {
        r.need("life")?;
        Some(TriggerPattern::LifeLost { who: by })
    } else if verb == if third { "draws" } else { "draw" } {
        if let Some(nth) = each_turn(r, "card") {
            let event = TriggerPattern::Drew { who: by };
            return Some(TriggerPattern::Nth {
                event: Box::new(event),
                nth,
            });
        }
        r.need("a card")?;
        Some(TriggerPattern::Drew { who: by })
    } else if verb == if third { "discards" } else { "discard" } {
        r.need("a card")?;
        Some(TriggerPattern::Discarded {
            what: Selector::default(),
            by,
        })
    } else if verb == if third { "cycles" } else { "cycle" } {
        let what = if r.words("another card") {
            Selector(vec![Pred::Other])
        } else if r.source() {
            Selector::this()
        } else {
            r.need("a card")?;
            Selector::default()
        };
        Some(TriggerPattern::Cycled { what, by })
    } else if verb == if third { "taps" } else { "tap" } {
        let what = subject(r)?;
        r.need("for mana")?;
        Some(TriggerPattern::TappedForMana { what, by })
    } else {
        None
    }
}

/// "your second card each turn", "your first spell each turn": which one of the turn, for a noun.
fn each_turn(r: &mut Reader<'_>, noun: &str) -> Option<u32> {
    r.attempt(|r| {
        r.need("your")?;
        let at = ORDINALS.iter().position(|word| r.word(word))?;
        r.need(noun)?;
        r.need("each turn")?;
        u32::try_from(at + 1).ok()
    })
}

/// The ordinals a count of a turn names.
const ORDINALS: [&str; 5] = ["first", "second", "third", "fourth", "fifth"];

/// The card itself, or an object with its article: "a creature you control", "another creature".
fn subject(r: &mut Reader<'_>) -> Option<Selector> {
    if r.source() {
        return Some(Selector::this());
    }
    // What an Equipment or an Aura is attached to.
    let attached = r.attempt(|r| {
        (r.word("equipped") || r.word("enchanted")).then_some(())?;
        r.word("creature").then_some(())
    });
    if attached.is_some() {
        return Some(Selector(vec![Pred::Is(Ref::Attached)]));
    }
    r.attempt(|r| {
        let article = r.word("a") || r.word("an");
        let selector = phrase(r, Form::Subject)?;
        // "another" stands for the article and for "other".
        (article != selector.0.contains(&Pred::Other)).then_some(selector)
    })
}

/// What the subject does: "enters", "dies", "leaves the battlefield", "attacks", and several of them
/// joined by "or".
fn verbs(r: &mut Reader<'_>, what: &Selector) -> Option<TriggerPattern> {
    let mut found = vec![verb(r, what)?];
    while r.word("or") {
        let next = verb(r, what)?;
        if found.contains(&next) {
            return None;
        }
        found.push(next);
    }
    if found.len() == 1 {
        found.pop()
    } else if found
        .iter()
        .any(|found| matches!(found, TriggerPattern::Nth { .. }))
    {
        // A count of one event is not part of a list of events.
        None
    } else {
        Some(TriggerPattern::AnyOf(found))
    }
}

fn verb(r: &mut Reader<'_>, what: &Selector) -> Option<TriggerPattern> {
    let (from, to) = if r.word("enters") {
        // The older wording, "enters the battlefield", says the same.
        r.words("the battlefield");
        (None, Some(ZoneKind::Battlefield))
    } else if r.word("dies") {
        (Some(ZoneKind::Battlefield), Some(ZoneKind::Graveyard))
    } else if r.words("leaves the battlefield") {
        (Some(ZoneKind::Battlefield), None)
    } else if r.word("attacks") {
        let attacks = TriggerPattern::Attacks { what: what.clone() };
        return Some(first_time(r, attacks));
    } else if r.words("becomes the target of a spell or ability") {
        return becomes_target(r, what);
    } else if r.words("is dealt damage") {
        let to = match what.0.as_slice() {
            [Pred::This] => Ref::This,
            [Pred::Is(Ref::Attached)] => Ref::Attached,
            _ => Ref::Each(what.clone()),
        };
        let damaged = TriggerPattern::DealsDamage {
            source: Selector::default(),
            to: Some(to),
            combat: None,
        };
        return Some(first_time(r, damaged));
    } else if r.word("blocks") {
        return Some(TriggerPattern::Blocks { what: what.clone() });
    } else if r.words("becomes blocked") {
        return Some(TriggerPattern::BecomesBlocked { what: what.clone() });
    } else if r.words("becomes tapped") {
        return Some(TriggerPattern::Tapped { what: what.clone() });
    } else if r.words("becomes untapped") {
        return Some(TriggerPattern::Untapped { what: what.clone() });
    } else if r.words("becomes attached to") {
        return Some(TriggerPattern::Attached {
            what: what.clone(),
            to: subject(r)?,
        });
    } else if r.word("deals") {
        return damage_dealt(r, what);
    } else {
        return None;
    };
    Some(TriggerPattern::ZoneChange {
        what: what.clone(),
        from,
        to,
    })
}

/// What follows "becomes the target of a spell or ability": "you control", "an opponent controls" or nothing, and
/// "for the first time each turn" (a count of the controller's own spells and abilities, so not for an opponent's).
fn becomes_target(r: &mut Reader<'_>, what: &Selector) -> Option<TriggerPattern> {
    let controller = if r.words("you control") {
        Some(Ref::You)
    } else if r.words("an opponent controls") {
        Some(Ref::Opponents)
    } else {
        None
    };
    let by = Selector(controller.iter().cloned().map(Pred::ControlledBy).collect());
    let target = TriggerPattern::BecomesTarget {
        what: what.clone(),
        by,
    };
    let counted = first_time(r, target);
    let opponents = controller == Some(Ref::Opponents);
    (!(opponents && matches!(counted, TriggerPattern::Nth { .. }))).then_some(counted)
}

/// The event as it stands, or its nth time of the turn when "for the first time each turn" follows: "for the second time
/// each turn".
fn first_time(r: &mut Reader<'_>, event: TriggerPattern) -> TriggerPattern {
    let nth = r.attempt(|r| {
        r.need("for the")?;
        let at = ORDINALS.iter().position(|word| r.word(word))?;
        r.need("time each turn")?;
        u32::try_from(at + 1).ok()
    });
    match nth {
        Some(nth) => TriggerPattern::Nth {
            event: Box::new(event),
            nth,
        },
        None => event,
    }
}

/// What follows "deals": "combat damage to a player", "damage to an opponent", "noncombat damage".
fn damage_dealt(r: &mut Reader<'_>, source: &Selector) -> Option<TriggerPattern> {
    let combat = if r.words("combat damage") {
        Some(true)
    } else if r.words("noncombat damage") {
        Some(false)
    } else {
        r.need("damage")?;
        None
    };
    let to = if r.words("to a player") {
        Some(Ref::EachPlayer)
    } else if r.words("to an opponent") {
        Some(Ref::Opponents)
    } else if r.words("to you") {
        Some(Ref::You)
    } else {
        None
    };
    Some(TriggerPattern::DealsDamage {
        source: source.clone(),
        to,
        combat,
    })
}

/// The condition of an intervening "if": "you have 40 or more life", "you're the monarch".
pub fn condition(r: &mut Reader<'_>) -> Option<Condition> {
    if r.words("you're the monarch") {
        return Some(Condition::Monarch(Ref::You));
    }
    if r.words("an opponent is the monarch") {
        return Some(Condition::Monarch(Ref::Opponents));
    }
    r.need("you have")?;
    let number = i32::try_from(r.number()?).ok()?;
    let op = if r.words("or more") {
        Cmp::Ge
    } else if r.words("or less") {
        Cmp::Le
    } else {
        return None;
    };
    r.word("life").then_some(())?;
    Some(Condition::Compare {
        left: Amount::Life(Ref::You),
        op,
        right: Amount::Fixed(number),
    })
}

/// The condition as the card words it, after "if".
pub fn condition_text(condition: &Condition) -> Option<String> {
    match condition {
        Condition::Monarch(Ref::You) => return Some("you're the monarch".to_owned()),
        Condition::Monarch(Ref::Opponents) => return Some("an opponent is the monarch".to_owned()),
        _ => {}
    }
    let Condition::Compare {
        left: Amount::Life(Ref::You),
        op,
        right: Amount::Fixed(number),
    } = condition
    else {
        return None;
    };
    let more = match op {
        Cmp::Ge => "more",
        Cmp::Le => "less",
        _ => return None,
    };
    Some(format!("you have {number} or {more} life"))
}

/// "a" or "an" for the words after it ("an Elf", "an 8/8 token", "a 1/1 token").
pub fn article(words: &str) -> &'static str {
    let vowel = words
        .chars()
        .next()
        .is_some_and(|first| "aeiou".contains(first.to_ascii_lowercase()));
    let eight = ["8", "11", "18"].iter().any(|number| {
        words
            .strip_prefix(number)
            .is_some_and(|rest| !rest.starts_with(|c: char| c.is_ascii_digit()))
    });
    if vowel || eight { "an" } else { "a" }
}

/// A subject with its article: "~", "a creature you control", "another Elf".
fn subject_text(what: &Selector) -> Option<String> {
    if what.0 == [Pred::This] {
        return Some("~".to_owned());
    }
    if what.0 == [Pred::Is(Ref::Attached)] {
        return Some("equipped creature".to_owned());
    }
    let text = phrase_text(what, Form::Subject)?;
    if what.0.contains(&Pred::Other) {
        Some(text)
    } else {
        Some(format!("{} {text}", article(&text)))
    }
}

/// "you cast", "an opponent casts", "a player casts" for the verb ("cast" or "sacrifice").
fn player_verb(by: &Ref, verb: &str) -> Option<String> {
    Some(match by {
        Ref::You => format!("you {verb}"),
        Ref::Opponents => format!("an opponent {verb}s"),
        Ref::EachPlayer => format!("a player {verb}s"),
        _ => return None,
    })
}

/// The subject of a pattern that is about one thing happening to it, and what happens.
fn happening(pattern: &TriggerPattern) -> Option<(Selector, String)> {
    let said = |what: &Selector, verb: &str| Some((what.clone(), verb.to_owned()));
    match pattern {
        TriggerPattern::ZoneChange { what, from, to } => {
            let verb = match (from, to) {
                (None, Some(ZoneKind::Battlefield)) => "enters",
                (Some(ZoneKind::Battlefield), Some(ZoneKind::Graveyard)) => "dies",
                (Some(ZoneKind::Battlefield), None) => "leaves the battlefield",
                _ => return None,
            };
            said(what, verb)
        }
        TriggerPattern::Attacks { what } => said(what, "attacks"),
        TriggerPattern::Blocks { what } => said(what, "blocks"),
        TriggerPattern::BecomesBlocked { what } => said(what, "becomes blocked"),
        TriggerPattern::Tapped { what } => said(what, "becomes tapped"),
        TriggerPattern::Untapped { what } => said(what, "becomes untapped"),
        TriggerPattern::Attached { what, to } if !what.is_empty() && !to.is_empty() => {
            said(what, &format!("becomes attached to {}", subject_text(to)?))
        }
        TriggerPattern::BecomesTarget { what, by } => {
            let whose = match by.0.as_slice() {
                [] => "",
                [Pred::ControlledBy(Ref::You)] => " you control",
                [Pred::ControlledBy(Ref::Opponents)] => " an opponent controls",
                _ => return None,
            };
            said(
                what,
                &format!("becomes the target of a spell or ability{whose}"),
            )
        }
        TriggerPattern::DealsDamage {
            source,
            to: Some(to),
            combat: None,
        } if source.is_empty() && !matches!(to, Ref::You | Ref::Opponents | Ref::EachPlayer) => {
            said(&damaged_subject(to)?, "is dealt damage")
        }
        TriggerPattern::DealsDamage { source, to, combat } => said(
            source,
            &format!("deals {}", damage_text(*combat, to.as_ref())?),
        ),
        _ => None,
    }
}

/// "combat damage to a player", "damage to an opponent", "noncombat damage".
fn damage_text(combat: Option<bool>, to: Option<&Ref>) -> Option<String> {
    let kind = match combat {
        Some(true) => "combat damage",
        Some(false) => "noncombat damage",
        None => "damage",
    };
    let to = match to {
        None => "",
        Some(Ref::EachPlayer) => " to a player",
        Some(Ref::Opponents) => " to an opponent",
        Some(Ref::You) => " to you",
        Some(_) => return None,
    };
    Some(format!("{kind}{to}"))
}

/// The step as the card words it after "the beginning of".
fn step_text(step: Step, whose: &Ref) -> Option<String> {
    if step == Step::BeginCombat {
        return match whose {
            Ref::You => Some("combat on your turn".to_owned()),
            Ref::EachPlayer => Some("each combat".to_owned()),
            _ => None,
        };
    }
    let (_, words) = STEPS.iter().find(|(named, _)| *named == step)?;
    let owner = match whose {
        Ref::You => "your",
        Ref::Opponents => "each opponent's",
        Ref::EachPlayer => "each",
        _ => return None,
    };
    Some(format!("{owner} {words}"))
}

/// The event as the card words it, from "When" to the end of the event.
pub fn pattern_text(pattern: &TriggerPattern) -> Option<String> {
    match pattern {
        TriggerPattern::StepBegins { step, whose } => {
            Some(format!("At the beginning of {}", step_text(*step, whose)?))
        }
        TriggerPattern::SpellCast { what, by } => {
            let mut preds = vec![Pred::Spell];
            preds.extend(what.0.iter().cloned());
            let spell = subject_text(&Selector(preds))?;
            Some(format!("Whenever {} {spell}", player_verb(by, "cast")?))
        }
        TriggerPattern::Sacrificed { what, by } => Some(format!(
            "Whenever {} {}",
            player_verb(by, "sacrifice")?,
            subject_text(what)?
        )),
        TriggerPattern::LifeGained { who } => {
            Some(format!("Whenever {} life", player_verb(who, "gain")?))
        }
        TriggerPattern::LifeLost { who } => {
            Some(format!("Whenever {} life", player_verb(who, "lose")?))
        }
        TriggerPattern::Drew { who } => {
            Some(format!("Whenever {} a card", player_verb(who, "draw")?))
        }
        TriggerPattern::Discarded { what, by } if what.is_empty() => {
            Some(format!("Whenever {} a card", player_verb(by, "discard")?))
        }
        TriggerPattern::Cycled { what, by } => {
            let card = match what.0.as_slice() {
                [] => "a card",
                [Pred::Other] => "another card",
                [Pred::This] if *by == Ref::You => return Some("When you cycle ~".to_owned()),
                _ => return None,
            };
            Some(format!("Whenever {} {card}", player_verb(by, "cycle")?))
        }
        TriggerPattern::TappedForMana { what, by } => Some(format!(
            "Whenever {} {} for mana",
            player_verb(by, "tap")?,
            subject_text(what)?
        )),
        TriggerPattern::CountersPut { what, counter } => {
            counter_text(what, counter.as_ref(), "put on")
        }
        TriggerPattern::CountersRemoved { what, counter } => {
            counter_text(what, counter.as_ref(), "removed from")
        }
        TriggerPattern::Nth { event, nth } => nth_text(event, *nth),
        TriggerPattern::Batch(inner) => batch_text(inner),
        TriggerPattern::GroupAttacks { what, at_least } => {
            let count = if *at_least == 1 {
                "one"
            } else {
                count_word(*at_least)?
            };
            Some(format!(
                "Whenever {count} or more {} attack",
                phrase_text(what, Form::All)?
            ))
        }
        TriggerPattern::AnyOf(patterns) => {
            let (first, rest) = patterns.split_first()?;
            let (what, verb) = happening(first)?;
            let mut verbs = vec![verb];
            for pattern in rest {
                let (other, verb) = happening(pattern)?;
                if other != what {
                    return None;
                }
                verbs.push(verb);
            }
            about(&what, &verbs)
        }
        _ => {
            let (what, verb) = happening(pattern)?;
            about(&what, &[verb])
        }
    }
}

/// What is dealt damage, as the subject of "is dealt damage": the card itself, what an Equipment or Aura is attached to
/// or the objects a selector names.
fn damaged_subject(to: &Ref) -> Option<Selector> {
    match to {
        Ref::This => Some(Selector::this()),
        Ref::Attached => Some(Selector(vec![Pred::Is(Ref::Attached)])),
        Ref::Each(what) => Some(what.clone()),
        _ => None,
    }
}

/// "Whenever you draw your second card each turn", "Whenever you cast your first spell each turn", "Whenever ~ attacks
/// for the first time each turn".
fn nth_text(event: &TriggerPattern, nth: u32) -> Option<String> {
    let ordinal = ORDINALS.get(usize::try_from(nth.checked_sub(1)?).ok()?)?;
    let (verb, noun) = match event {
        TriggerPattern::Drew { who: Ref::You } => ("draw", "card"),
        TriggerPattern::SpellCast { what, by: Ref::You } if what.is_empty() => ("cast", "spell"),
        TriggerPattern::Attacks { .. }
        | TriggerPattern::BecomesTarget { .. }
        | TriggerPattern::DealsDamage { .. } => {
            let counted = pattern_text(event)?;
            return Some(format!("{counted} for the {ordinal} time each turn"));
        }
        _ => return None,
    };
    Some(format!(
        "Whenever you {verb} your {ordinal} {noun} each turn"
    ))
}

/// "Whenever one or more creatures you control deal combat damage to a player", "Whenever one or more cards are put
/// into your graveyard from your library".
fn batch_text(inner: &TriggerPattern) -> Option<String> {
    match inner {
        TriggerPattern::DealsDamage { source, to, combat } => Some(format!(
            "Whenever one or more {} deal {}",
            phrase_text(source, Form::All)?,
            damage_text(*combat, to.as_ref())?
        )),
        TriggerPattern::ZoneChange {
            what,
            from: Some(ZoneKind::Library),
            to: Some(ZoneKind::Graveyard),
        } if what.0 == [Pred::OwnedBy(Ref::You)] => Some(
            "Whenever one or more cards are put into your graveyard from your library".to_owned(),
        ),
        _ => None,
    }
}

/// "Whenever a +1/+1 counter is put on ~", "Whenever a counter is removed from a creature you control".
fn counter_text(what: &Selector, counter: Option<&CounterKind>, moved: &str) -> Option<String> {
    let place = if what.is_empty() {
        "a permanent".to_owned()
    } else {
        subject_text(what)?
    };
    Some(format!(
        "Whenever {} is {moved} {place}",
        counters::trigger_kind_text(counter)?
    ))
}

/// "When ~ enters", "Whenever a land you control enters", "Whenever ~ enters or attacks": "when" is
/// for the card itself entering, dying or leaving, "whenever" for the rest.
fn about(what: &Selector, verbs: &[String]) -> Option<String> {
    let once = what.0 == [Pred::This]
        && verbs
            .iter()
            .all(|verb| matches!(verb.as_str(), "enters" | "dies" | "leaves the battlefield"));
    Some(format!(
        "{} {} {}",
        if once { "When" } else { "Whenever" },
        subject_text(what)?,
        verbs.join(" or ")
    ))
}
