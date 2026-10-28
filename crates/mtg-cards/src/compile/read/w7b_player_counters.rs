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

/// "you get two rad counters": the counters put on you, with the verb that agrees with "you".
pub(super) fn w7b_you_get(effect: &Effect) -> Option<String> {
    match effect {
        Effect::PutCounters {
            on: Ref::You,
            counter,
            count,
        } if w7b_player_kind(counter) => {
            Some(format!("you get {}", counters::text(counter, count)?))
        }
        _ => None,
    }
}
