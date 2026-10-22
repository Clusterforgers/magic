//! A condition on the effects after it: "If there are five or more hatchling counters on ~, remove all of them and
//! transform ~." is an `Effect::If`, also after "Then" when the sentence before it acted on the same card. The
//! condition counts the counters on the card; the effects are steps joined by "and" or one sentence. The lists
//! below add this part's shapes (transform, the look and reveal, the sentence itself) to those of `effects`.

use super::amounts::{count_word, number_word};
use super::counters;
use super::effects::{self, Template};
use super::reveal;
use super::tokens::Reader;
use super::transform;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::{Amount, Cmp, Condition, CounterKind};

/// The sentence shapes of this part, tried before those of `effects` (a look at the top card is read with the
/// reveal that follows it, not alone).
pub const TEMPLATES: [Template; 3] = [reveal::look_reveal, if_sentence, transform::transform];

/// The shapes that begin with a verb, which "you may" can precede.
pub const IMPERATIVES: [Template; 1] = [transform::transform];

/// "If there are five or more hatchling counters on ~, remove all of them and transform ~.", also "Then if ...".
pub fn if_sentence(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.word("then");
    r.need("if")?;
    let condition = condition(r)?;
    r.punct(',').then_some(())?;
    let then = consequence(r, &condition)?;
    Some(vec![Effect::If {
        condition,
        then,
        otherwise: Vec::new(),
    }])
}

/// "there are five or more hatchling counters on ~", "... two or fewer ... on it": a count of the counters on the
/// card, which "it" names when the sentence before acted on the card.
fn condition(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("there are")?;
    let number = number_word(r.take_word()?).filter(|number| *number > 1)?;
    let op = if r.words("or more") {
        Cmp::Ge
    } else {
        r.need("or fewer")?;
        Cmp::Le
    };
    let counter = counters::kind(r)?;
    r.need("counters on")?;
    if !(r.source() || (r.it == Some(Ref::This) && r.word("it"))) {
        return None;
    }
    // The card is what "it" means in the effects that follow.
    r.it = Some(Ref::This);
    Some(Condition::Compare {
        left: Amount::Counters {
            on: Ref::This,
            counter,
        },
        op,
        right: Amount::Fixed(i32::try_from(number).ok()?),
    })
}

/// What the effects after a condition do: steps joined by "and" ("remove all of them and transform ~") or one
/// sentence, up to the end of the sentence.
pub(super) fn consequence(r: &mut Reader<'_>, condition: &Condition) -> Option<Vec<Effect>> {
    let ends = |r: &Reader<'_>| r.at_punct('.') || r.end();
    r.attempt(|r| {
        let found = steps(r, condition)?;
        ends(r).then_some(found)
    })
    .or_else(|| {
        r.attempt(|r| {
            let found = effects::one_sentence(r)?;
            ends(r).then_some(found)
        })
    })
}

/// Steps joined by "and" or in a list.
fn steps(r: &mut Reader<'_>, condition: &Condition) -> Option<Vec<Effect>> {
    let mut found = step(r, condition)?;
    while let Some(more) = r.attempt(|r| {
        let listed = r.punct(',');
        let joined = r.word("and");
        (listed || joined).then_some(())?;
        step(r, condition)
    }) {
        found.extend(more);
    }
    Some(found)
}

/// One step: "remove all of them" for the counters the condition counts, or a verb of the imperative shapes.
fn step(r: &mut Reader<'_>, condition: &Condition) -> Option<Vec<Effect>> {
    if let Some(counter) = counted(condition)
        && r.words("remove all of them")
    {
        return Some(vec![Effect::RemoveCounters {
            from: Ref::This,
            count: Amount::Counters {
                on: Ref::This,
                counter: counter.clone(),
            },
            counter: counter.clone(),
        }]);
    }
    effects::IMPERATIVES
        .into_iter()
        .chain(IMPERATIVES)
        .find_map(|template| r.attempt(template))
}

/// The kind of counter the condition counts on the card.
fn counted(condition: &Condition) -> Option<&CounterKind> {
    match condition {
        Condition::Compare {
            left:
                Amount::Counters {
                    on: Ref::This,
                    counter,
                },
            ..
        } => Some(counter),
        _ => None,
    }
}

/// The condition as the card words it, after "if".
pub fn condition_text(condition: &Condition) -> Option<String> {
    if *condition == Condition::Performed {
        return Some("you do".to_owned());
    }
    let Condition::Compare {
        left: Amount::Counters {
            on: Ref::This,
            counter,
        },
        op,
        right: Amount::Fixed(number),
    } = condition
    else {
        return None;
    };
    let number = u32::try_from(*number).ok().filter(|number| *number > 1)?;
    let more = match op {
        Cmp::Ge => "more",
        Cmp::Le => "fewer",
        _ => return None,
    };
    Some(format!(
        "there are {} or {more} {} counters on ~",
        count_word(number)?,
        counters::kind_text(counter)?
    ))
}

/// "remove all of them": the effect takes away every counter the condition counted.
pub fn remove_all_text(condition: &Condition, effect: &Effect) -> Option<String> {
    let counter = counted(condition)?;
    let all = Effect::RemoveCounters {
        from: Ref::This,
        count: Amount::Counters {
            on: Ref::This,
            counter: counter.clone(),
        },
        counter: counter.clone(),
    };
    (*effect == all).then(|| "remove all of them".to_owned())
}

/// Every way the sentences of an ability can be worded: "Then if" for "If" after another sentence, and "it" for the
/// card where a sentence has just spoken of it. The data records neither, so each wording is written; the first is
/// the plain one.
pub fn wordings(sentences: &[String]) -> Vec<Vec<String>> {
    let mut all: Vec<Vec<String>> = vec![Vec::new()];
    for (at, sentence) in sentences.iter().enumerate() {
        let options = sentence_wordings(sentence, at > 0);
        all = all
            .into_iter()
            .flat_map(|so_far| {
                options.iter().map(move |option| {
                    let mut next = so_far.clone();
                    next.push(option.clone());
                    next
                })
            })
            .take(64)
            .collect();
    }
    all
}

/// The wordings of one sentence; `later` says a sentence comes before it.
fn sentence_wordings(sentence: &str, later: bool) -> Vec<String> {
    let mut out = vec![sentence.to_owned()];
    if let Some(head) = sentence.strip_suffix("transform ~") {
        out.push(format!("{head}transform it"));
    }
    if sentence.starts_with("if there are ")
        && let Some((counted, rest)) = sentence.split_once(", ")
    {
        let it = |text: &str| text.replace('~', "it");
        out.push(format!("{}, {rest}", it(counted)));
        out.push(format!("{counted}, {}", it(rest)));
        out.push(format!("{}, {}", it(counted), it(rest)));
    }
    for (from, to) in COPY_WORDINGS {
        let others: Vec<String> = out
            .iter()
            .filter_map(|text| replace_phrase(text, from, to))
            .collect();
        out.extend(others);
    }
    if later && sentence.starts_with("if ") {
        let then: Vec<String> = out.iter().map(|text| format!("then {text}")).collect();
        out.extend(then);
    }
    out
}

/// The words of a copy that a card words two ways: the spell that was cast is "it" or "that spell", the exception of a
/// copy of a spell has "that" or not, and a creature that attacked becomes a copy as "it" or by its name.
const COPY_WORDINGS: [(&str, &str); 3] = [
    ("copy it", "copy that spell"),
    (", except that the copy", ", except the copy"),
    ("~ becomes a copy of", "it becomes a copy of"),
];

/// The text with the first `from` that stands alone as `to`.
fn replace_phrase(text: &str, from: &str, to: &str) -> Option<String> {
    let at = text.find(from)?;
    let (before, after) = (&text[..at], &text[at + from.len()..]);
    let word = |c: char| c.is_alphanumeric();
    let alone = !(from.starts_with(word) && before.ends_with(word))
        && !(from.ends_with(word) && after.starts_with(word));
    alone.then(|| format!("{before}{to}{after}"))
}
