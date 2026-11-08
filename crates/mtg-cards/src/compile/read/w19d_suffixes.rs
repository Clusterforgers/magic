//! Suffixes of a noun phrase about combat and damage: "creature blocking it", "creature it's blocking", "creature blocking
//! or blocked by it", "creature that was dealt damage this turn", "creature that's attacking you". The words follow the
//! noun, read and written like the other suffixes (`selectors::suffix`); the card is "~" in the written words and "it"
//! where an earlier word of the sentence names it.

use super::selectors::Form;
use super::tokens::Reader;
use crate::ability::Ability;
use crate::refs::{Pred, Ref};
use crate::trigger::TriggerPattern;

/// The card itself, as "it" or by its name.
fn w19d_itself(r: &mut Reader<'_>) -> bool {
    r.word("it") || r.source()
}

/// The suffix at the reader, when it is one of the above.
pub(super) fn w19d_suffix(r: &mut Reader<'_>) -> Option<Pred> {
    if let Some(pred) = r.attempt(|r| {
        r.need("blocking or blocked by")?;
        w19d_itself(r)
            .then(|| Pred::AnyOf(vec![Pred::Blocks(Ref::This), Pred::BlockedBy(Ref::This)]))
    }) {
        return Some(pred);
    }
    if r.attempt(|r| {
        r.need("blocking")?;
        w19d_itself(r).then_some(())
    })
    .is_some()
    {
        return Some(Pred::Blocks(Ref::This));
    }
    if r.attempt(|r| {
        if r.words("it's") || (r.source() && r.word("is")) {
            r.need("blocking")
        } else if r.word("blocked") {
            r.need("by")?;
            w19d_itself(r).then_some(())
        } else {
            None
        }
    })
    .is_some()
    {
        return Some(Pred::BlockedBy(Ref::This));
    }
    if r.words("that's attacking you") || r.words("that are attacking you") {
        return Some(Pred::AttackingPlayer(Ref::You));
    }
    None
}

/// The wordings with "it" for the card where an earlier word of the sentence names it.
pub(super) fn add(texts: &mut Vec<String>) {
    // Each rewording, and whether it needs an earlier "~" for "it" to name.
    const REWORDINGS: [(&str, &str, bool); 9] = [
        (" blocking ~", " blocking it", true),
        (" blocking ~", " that's blocking ~", false),
        (" blocking ~", " that's blocking it", true),
        (" blocked by ~", " blocked by it", true),
        (" blocked by ~", " ~ is blocking", false),
        (" blocked by ~", " it's blocking", true),
        (" blocked by ~", " that ~ is blocking", false),
        (" blocked by ~", " that it's blocking", true),
        (" that's attacking you", " attacking you", false),
    ];
    let mut found = Vec::new();
    for text in texts.iter() {
        for (from, to, named) in REWORDINGS {
            let Some(at) = text.rfind(from) else {
                continue;
            };
            if !named || text[..at].contains('~') {
                found.push(format!("{}{to}{}", &text[..at], &text[at + from.len()..]));
            }
        }
    }
    // "..., you put a stun counter on target creature attacking you." is the older "..., choose target creature attacking
    // you. Put a stun counter on that creature."
    for text in texts.iter() {
        let Some((head, rest)) = text.split_once(", you put ") else {
            continue;
        };
        if let Some(counters) = rest.strip_suffix(" on target creature that's attacking you.") {
            found.push(format!(
                "{head}, choose target creature attacking you. Put {counters} on that creature."
            ));
        }
    }
    // A trigger about a spell alone names that spell, not "that spell or ability".
    for text in texts.iter() {
        if text.contains("target of a spell, that spell or ability's controller") {
            found.push(text.replace(
                "target of a spell, that spell or ability's controller",
                "target of a spell, that spell's controller",
            ));
        }
    }
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// The words of one of the suffixes above, after "target" only (the other phrases count, join and pluralize the words in
/// ways of their own); `None` for any other predicate or phrase.
pub(super) fn w19d_suffix_text(pred: &Pred, form: Form) -> Option<String> {
    (form == Form::Target).then_some(())?;
    Some(match pred {
        Pred::Blocks(Ref::This) => "blocking ~".to_owned(),
        Pred::BlockedBy(Ref::This) => "blocked by ~".to_owned(),
        Pred::AnyOf(any) if *any == [Pred::Blocks(Ref::This), Pred::BlockedBy(Ref::This)] => {
            "blocking or blocked by ~".to_owned()
        }
        Pred::DamagedThisTurn => "that was dealt damage this turn".to_owned(),
        Pred::AttackingPlayer(Ref::You) => "that's attacking you".to_owned(),
        _ => return None,
    })
}

/// Whether the text of a triggered ability may say "that spell or ability's controller": only after a trigger about what
/// targets the card, whose "spell or ability" it is.
pub(super) fn w19d_targeting_controller(ability: &Ability, text: &str) -> bool {
    !text.contains("that spell or ability's controller")
        || matches!(
            ability,
            Ability::Triggered {
                when: TriggerPattern::BecomesTarget { .. },
                ..
            }
        )
}

/// The suffixes that open with "that" and say what happened go last, after who controls the object and what it has:
/// "creature an opponent controls that was dealt damage this turn", "creature without flying that's attacking you".
pub(super) fn last(mut suffixes: Vec<String>) -> Vec<String> {
    suffixes.sort_by_key(|suffix| {
        suffix == "that was dealt damage this turn" || suffix == "that's attacking you"
    });
    suffixes
}
