//! r55-06 changes of the cost of exhaust and boast abilities and of buyback costs, read and written back: "Exhaust
//! abilities of other permanents you control cost {2} less to activate.", "Boast abilities you activate cost {1} less to
//! activate for each Dragon you control." and "Buyback costs cost {2} less.".

use super::tokens::{Reader, tokenize};
use crate::ability::{Ability, ActivationKind, CostChange};
use crate::refs::{Pred, Ref, Selector};
use crate::value::Amount;

/// The subjects of the sentences: the words, the kind and the sources whose abilities change.
fn r5506_subjects() -> [(&'static str, ActivationKind, Selector); 5] {
    let you = Pred::ControlledBy(Ref::You);
    [
        (
            "Exhaust abilities of other permanents you control cost",
            ActivationKind::Exhaust,
            Selector(vec![Pred::Other, you.clone()]),
        ),
        (
            "Exhaust abilities you activate cost",
            ActivationKind::Exhaust,
            Selector(vec![you.clone()]),
        ),
        (
            "Boast abilities you activate cost",
            ActivationKind::Boast,
            Selector(vec![you.clone()]),
        ),
        (
            "Boast abilities of other creatures you control cost",
            ActivationKind::Boast,
            Selector(vec![Pred::Other, you]),
        ),
        (
            "Buyback costs cost",
            ActivationKind::Buyback,
            Selector::default(),
        ),
    ]
}

/// Reads one of the sentences; none for any other text.
pub(super) fn r5506_cost_read(text: &str) -> Option<Ability> {
    let tokens = tokenize(text);
    let mut r = Reader::new(&tokens, false);
    let (kind, applies_to) =
        r5506_subjects()
            .into_iter()
            .find_map(|(words, kind, applies_to)| {
                r.words(&words.to_lowercase()).then_some((kind, applies_to))
            })?;
    let (mana, x) = super::statics::cost_mana(&mut r)?;
    (!x).then_some(())?;
    let change = if r.word("more") {
        CostChange::Increase
    } else if r.word("less") {
        CostChange::Reduce
    } else {
        return None;
    };
    let mut per = Amount::one();
    if kind != ActivationKind::Buyback {
        r.words("to activate").then_some(())?;
        if let Some(counted) = r.attempt(super::amounts::for_each) {
            per = counted;
        }
    }
    r.punct('.');
    r.end().then_some(())?;
    Some(Ability::CostModifier {
        targeting: None,
        applies_to,
        change,
        mana,
        per,
        condition: None,
        activating: Some(kind),
        least: 0,
        life: 0,
    })
}

/// Adds the sentence of a change of these kinds.
pub(super) fn r5506_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::CostModifier {
        targeting: None,
        applies_to,
        change,
        mana,
        per,
        condition: None,
        activating: Some(kind),
        least: 0,
        life: 0,
    } = ability
    else {
        return;
    };
    let more = match change {
        CostChange::Increase => "more",
        CostChange::Reduce => "less",
    };
    let each = if per.is_one() {
        String::new()
    } else if *kind == ActivationKind::Buyback {
        return;
    } else {
        let Some(counted) = super::amounts::for_each_text(per) else {
            return;
        };
        format!(" for each {counted}")
    };
    for (words, wanted, sources) in r5506_subjects() {
        if wanted != *kind || sources != *applies_to {
            continue;
        }
        if *kind == ActivationKind::Buyback {
            texts.push(format!("{words} {mana} {more}."));
        } else {
            texts.push(format!("{words} {mana} {more} to activate{each}."));
        }
    }
}
