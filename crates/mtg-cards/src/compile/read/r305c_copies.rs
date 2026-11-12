//! Wordings of a permanent that enters as a copy with counters: "except it enters with an additional +1/+1 counter on
//! it if it's a creature", "except it enters with X additional +1/+1 counters on it".

use crate::modify::Modification;
use crate::refs::{Pred, Selector};
use crate::trigger::CopyCounters;
use crate::value::Amount;
use mtg_core::card::CardType;

/// The wordings of `AsCopy` with counters: the plain wordings of the copy with the counters joined to its exceptions.
pub(super) fn r305c_counter_copy_texts(
    of: &Selector,
    except: &[Modification],
    tapped: bool,
    until_end_of_turn: bool,
    counters: &[CopyCounters],
) -> Vec<String> {
    if counters.is_empty() || tapped || until_end_of_turn {
        return Vec::new();
    }
    let noun = if of.0.contains(&Pred::Type(CardType::Creature)) {
        "creature"
    } else {
        "permanent"
    };
    let mut phrases: Vec<Vec<String>> = Vec::new();
    for entry in counters {
        let Some(words) = r305c_counter_phrases(entry, noun) else {
            return Vec::new();
        };
        phrases.push(words);
    }
    let plain = super::copies::enters_texts(of, &[], false, false);
    let rests: Vec<Option<String>> = if except.is_empty() {
        vec![None]
    } else {
        super::copies::enters_texts(of, except, false, false)
            .into_iter()
            .filter_map(|text| {
                plain
                    .iter()
                    .find_map(|head| text.strip_prefix(&format!("{head}, except ")))
                    .map(|rest| Some(rest.to_owned()))
            })
            .collect()
    };
    let mut texts = Vec::new();
    for head in &plain {
        for rest in &rests {
            for parts in r305c_product(&phrases) {
                let mut all = parts;
                all.extend(rest.clone());
                texts.push(format!("{head}, except {}", r305c_join(&all)));
            }
        }
    }
    texts
}

/// "it enters with an additional +1/+1 counter on it if it's a creature", with and without "additional".
fn r305c_counter_phrases(entry: &CopyCounters, noun: &str) -> Option<Vec<String>> {
    let kind = entry.counter.name();
    let article = if kind.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    let condition = r305c_condition(&entry.of, noun)?;
    let mut out = Vec::new();
    for additional in [true, false] {
        let count = match &entry.count {
            Amount::Fixed(1) if additional => format!("an additional {kind} counter"),
            Amount::Fixed(1) => format!("{article} {kind} counter"),
            Amount::Fixed(n) => {
                let word = super::amounts::count_word(u32::try_from(*n).ok()?)?;
                let extra = if additional { " additional" } else { "" };
                format!("{word}{extra} {kind} counters")
            }
            Amount::X => {
                let extra = if additional { " additional" } else { "" };
                format!("X{extra} {kind} counters")
            }
            _ => return None,
        };
        out.push(format!("it enters with {count} on it{condition}"));
    }
    Some(out)
}

/// " if it's a creature", " if you control that creature", or nothing.
fn r305c_condition(of: &Selector, noun: &str) -> Option<String> {
    match of.0.as_slice() {
        [] => Some(String::new()),
        [Pred::Type(CardType::Creature)] => Some(" if it's a creature".to_owned()),
        [Pred::Type(CardType::Planeswalker)] => Some(" if it's a planeswalker".to_owned()),
        [Pred::Type(CardType::Artifact)] => Some(" if it's an artifact".to_owned()),
        [Pred::ControlledBy(crate::refs::Ref::You)] => Some(format!(" if you control that {noun}")),
        _ => None,
    }
}

/// Every choice of one wording per counter entry.
fn r305c_product(phrases: &[Vec<String>]) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = vec![Vec::new()];
    for options in phrases {
        out = out
            .into_iter()
            .flat_map(|done| {
                options.iter().map(move |option| {
                    let mut next = done.clone();
                    next.push(option.clone());
                    next
                })
            })
            .collect();
    }
    out
}

/// "a", "a and b", "a, b, and c".
fn r305c_join(parts: &[String]) -> String {
    match parts {
        [one] => one.clone(),
        [first, second] => format!("{first} and {second}"),
        [init @ .., last] => format!("{}, and {last}", init.join(", ")),
        [] => String::new(),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: copies that enter with counters are written as the cards print them, and near misses are not.
    use super::*;
    use crate::refs::Ref;
    use crate::value::CounterKind;
    use rstest::rstest;

    fn creature() -> Selector {
        Selector(vec![Pred::Type(CardType::Creature)])
    }

    fn plus(count: Amount, of: Vec<Pred>) -> CopyCounters {
        CopyCounters {
            counter: CounterKind::PlusOnePlusOne,
            count,
            of: Selector(of),
        }
    }

    #[rstest]
    #[case::x_counters(vec![plus(Amount::X, vec![])], "you may have ~ enter as a copy of any creature on the battlefield, except it enters with X additional +1/+1 counters on it", true)]
    #[case::shield_if_yours(vec![CopyCounters { counter: CounterKind::Other("shield".into()), count: Amount::Fixed(1), of: Selector(vec![Pred::ControlledBy(Ref::You)]) }], "you may have ~ enter as a copy of any creature on the battlefield, except it enters with a shield counter on it if you control that creature", true)]
    #[case::two_counters(vec![plus(Amount::Fixed(2), vec![])], "you may have ~ enter as a copy of any creature on the battlefield, except it enters with two additional +1/+1 counters on it", true)]
    #[case::near_miss_count(vec![plus(Amount::Fixed(2), vec![])], "you may have ~ enter as a copy of any creature on the battlefield, except it enters with three additional +1/+1 counters on it", false)]
    #[case::near_miss_condition(vec![plus(Amount::Fixed(1), vec![Pred::Type(CardType::Creature)])], "you may have ~ enter as a copy of any creature on the battlefield, except it enters with an additional +1/+1 counter on it", false)]
    fn r305c_copy_counters_read_back(
        #[case] counters: Vec<CopyCounters>,
        #[case] text: &str,
        #[case] written: bool,
    ) {
        let texts = r305c_counter_copy_texts(&creature(), &[], false, false, &counters);
        assert_eq!(texts.iter().any(|t| t == text), written, "{texts:#?}");
    }

    #[test]
    fn r305c_no_counters_is_no_wording() {
        assert_eq!(
            r305c_counter_copy_texts(&creature(), &[], false, false, &[]),
            Vec::<String>::new()
        );
    }
}
