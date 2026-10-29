//! A stat compared with the counters on the source: "Destroy each nonland permanent with mana value less than or equal
//! to the number of time counters on ~" (The Moment) and "one or more other cards are put into exile from anywhere"
//! (The War Doctor); the writer's side of each is beside its reader.

use super::counters;
use super::selectors::{Form, phrase_text};
use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Cmp, StatKind};

/// "less than or equal to the number of time counters on ~" after the stat's name: the stat at most the counters of a
/// kind on the source.
pub(super) fn w15b_at_most_counters(r: &mut Reader<'_>, stat: StatKind) -> Option<Pred> {
    r.attempt(|r| {
        r.need("less than or equal to the number of")?;
        let to = counters::counted_plural(r)?;
        matches!(to, Amount::Counters { .. }).then_some(Pred::Compare {
            stat,
            op: Cmp::Le,
            to,
        })
    })
}

/// The writer of `w15b_at_most_counters`: the words after "with" and the stat's name; `None` for any other comparison.
pub(super) fn w15b_at_most_counters_text(op: Cmp, to: &Amount) -> Option<String> {
    let Amount::Counters {
        on: Ref::This,
        counter,
    } = to
    else {
        return None;
    };
    (op == Cmp::Le).then_some(())?;
    Some(format!(
        "less than or equal to the number of {} counters on ~",
        counters::kind_text(counter)?
    ))
}

/// The cards "cards put into exile from anywhere" counts: not tokens, and not the source when it says "other".
fn w15b_exiled_cards(other: bool) -> Selector {
    let not_token = Pred::Not(Box::new(Pred::Token));
    Selector(if other {
        vec![Pred::Other, not_token]
    } else {
        vec![not_token]
    })
}

/// "other cards are put into exile from anywhere" after "one or more" (The War Doctor): the cards exiled together,
/// from any zone, by anything.
pub(super) fn w15b_exiled_batch(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    let other = r.word("other");
    r.need("cards are put into exile from anywhere")?;
    Some(TriggerPattern::Batch(Box::new(TriggerPattern::ZoneChange {
        what: w15b_exiled_cards(other),
        from: None,
        to: Some(ZoneKind::Exile),
    })))
}

/// The writer of `w15b_exiled_batch`, from the group of a batch of moves into exile.
pub(super) fn w15b_exiled_batch_text(what: &Selector) -> Option<String> {
    let other = if *what == w15b_exiled_cards(true) {
        "other "
    } else if *what == w15b_exiled_cards(false) {
        ""
    } else {
        return None;
    };
    Some(format!(
        "Whenever one or more {other}cards are put into exile from anywhere"
    ))
}

/// Adds the wordings of the cards of this module that the renderers do not write: "destroy each", "Then sacrifice ~"
/// and the counters "on it" of the source that deals damage.
pub(super) fn w15b_wordings(ability: &Ability, texts: &mut Vec<String>) {
    w15b_destroy_each(ability, texts);
    w15b_then_sacrifice(ability, texts);
    w15b_counters_on_it(texts);
}

/// "it deals damage equal to the number of time counters on it": the source that deals the damage is "it" in the
/// amount too.
fn w15b_counters_on_it(texts: &mut Vec<String>) {
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("deals damage equal to the number of"))
        .filter_map(|text| {
            let (head, tail) = text.split_once("deals damage equal to the number of")?;
            let (counted, rest) = tail.split_once(" counters on ~ ")?;
            Some(format!(
                "{head}deals damage equal to the number of{counted} counters on it {rest}"
            ))
        })
        .collect();
    texts.extend(said);
}

/// Adds the wordings that say "destroy each" and the singular in place of "destroy all" and the plural: "Destroy each
/// nonland permanent with mana value ...".
fn w15b_destroy_each(ability: &Ability, texts: &mut Vec<String>) {
    for effect in ability.effects() {
        let Effect::Destroy {
            what: Ref::Each(selector),
            ..
        } = effect
        else {
            continue;
        };
        let (Some(all), Some(each)) = (
            phrase_text(selector, Form::All),
            phrase_text(selector, Form::Each),
        ) else {
            continue;
        };
        let mut said = Vec::new();
        for (long, short) in [("Destroy all", "Destroy each"), ("destroy all", "destroy each")] {
            let (long, short) = (format!("{long} {all}"), format!("{short} {each}"));
            said.extend(
                texts
                    .iter()
                    .filter(|text| text.contains(&long))
                    .map(|text| text.replacen(&long, &short, 1)),
            );
        }
        texts.extend(said);
    }
}

/// "Then sacrifice ~." after a mass destruction (The Moment): the sacrifice is the step after it.
fn w15b_then_sacrifice(ability: &Ability, texts: &mut Vec<String>) {
    let follows = ability.effects().windows(2).any(|pair| {
        matches!(pair, [Effect::Destroy { what: Ref::Each(_), .. }, Effect::Sacrifice { who: Ref::You, filter, count }]
            if *filter == Selector::this() && count.is_one())
    });
    if !follows {
        return;
    }
    let then: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(". Sacrifice ~."))
        .map(|text| text.replacen(". Sacrifice ~.", ". Then sacrifice ~.", 1))
        .collect();
    texts.extend(then);
}
