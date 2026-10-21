//! Triggers: the event a triggered ability waits for, and the condition of an intervening "if",
//! read from "when ~ enters", "whenever you cast a creature spell" or "at the beginning of your
//! upkeep" and written back the same way.

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::{Step, TriggerPattern};
use crate::value::{Amount, Cmp, Condition};

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
    r.attempt(player_event).or_else(|| {
        let what = subject(r)?;
        verbs(r, &what)
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
    } else {
        None
    }
}

/// The card itself, or an object with its article: "a creature you control", "another creature".
fn subject(r: &mut Reader<'_>) -> Option<Selector> {
    if r.source() {
        return Some(Selector::this());
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
        return Some(TriggerPattern::Attacks { what: what.clone() });
    } else {
        return None;
    };
    Some(TriggerPattern::ZoneChange {
        what: what.clone(),
        from,
        to,
    })
}

/// The condition of an intervening "if": "you have 40 or more life".
pub fn condition(r: &mut Reader<'_>) -> Option<Condition> {
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
fn happening(pattern: &TriggerPattern) -> Option<(&Selector, &'static str)> {
    match pattern {
        TriggerPattern::ZoneChange { what, from, to } => {
            let verb = match (from, to) {
                (None, Some(ZoneKind::Battlefield)) => "enters",
                (Some(ZoneKind::Battlefield), Some(ZoneKind::Graveyard)) => "dies",
                (Some(ZoneKind::Battlefield), None) => "leaves the battlefield",
                _ => return None,
            };
            Some((what, verb))
        }
        TriggerPattern::Attacks { what } => Some((what, "attacks")),
        _ => None,
    }
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
            about(what, &verbs)
        }
        _ => {
            let (what, verb) = happening(pattern)?;
            about(what, &[verb])
        }
    }
}

/// "When ~ enters", "Whenever a land you control enters", "Whenever ~ enters or attacks": "when" is
/// for the card itself entering, dying or leaving, "whenever" for the rest.
fn about(what: &Selector, verbs: &[&str]) -> Option<String> {
    let once = what.0 == [Pred::This]
        && verbs
            .iter()
            .all(|verb| matches!(*verb, "enters" | "dies" | "leaves the battlefield"));
    Some(format!(
        "{} {} {}",
        if once { "When" } else { "Whenever" },
        subject_text(what)?,
        verbs.join(" or ")
    ))
}
