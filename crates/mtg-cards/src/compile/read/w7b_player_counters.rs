//! Counters a player gets: "you get two rad counters", "each opponent gets a poison counter", "target player gets X
//! rad counters", read after the subject and its verb and written back the same way.

use super::counters;
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::{Amount, CounterKind};

/// The kinds of counter a player holds that the wording names by word (energy is written with its symbol).
const W7B_PLAYER_KINDS: [&str; 3] = ["rad", "poison", "experience"];

/// Whether `counter` is a kind a player gets by word.
pub(super) fn w7b_player_kind(counter: &CounterKind) -> bool {
    W7B_PLAYER_KINDS.contains(&counter.name())
}

/// "two rad counters", "a poison counter", "X rad counters", "that many rad counters" (in a trigger): the counters
/// `who` gets.
pub(super) fn w7b_read(r: &mut Reader<'_>, who: Ref) -> Option<Effect> {
    let (counter, count) = if r.triggered && r.words("that many") {
        let counter = counters::kind(r)?;
        r.need("counters")?;
        (counter, Amount::Triggering)
    } else {
        counters::entering(r)?
    };
    w7b_player_kind(&counter).then_some(Effect::PutCounters {
        on: who,
        counter,
        count,
    })
}

/// The wordings with "they get" for "that player gets" counters by word: "..., they get four rad counters."
pub(super) fn w7b_they_wordings(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter(|text| {
            W7B_PLAYER_KINDS
                .iter()
                .any(|kind| text.contains(&format!(" {kind} counter")))
        })
        .filter(|text| text.contains(", that player gets "))
        .map(|text| text.replacen(", that player gets ", ", they get ", 1))
        .collect()
}

/// "you get two rad counters", "that player gets four rad counters": the counters put on you or on the player of the
/// trigger, with the verb that agrees with the subject.
pub(super) fn w7b_player_gets(effect: &Effect) -> Option<String> {
    let Effect::PutCounters { on, counter, count } = effect else {
        return None;
    };
    let subject = match on {
        Ref::You => "you get",
        Ref::Triggering => "that player gets",
        _ => return None,
    };
    w7b_player_kind(counter).then_some(())?;
    Some(format!("{subject} {}", counters::text(counter, count)?))
}
