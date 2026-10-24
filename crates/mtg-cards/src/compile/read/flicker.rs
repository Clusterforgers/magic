//! A permanent that exiles itself and comes back: "Exile ~, then return it to the battlefield transformed under your
//! control." is an `Exile` that binds the card as the object it becomes in exile and a `Move` of that object onto the
//! battlefield, with its back face up when the sentence says transformed (CR 712.14a). The last chapter of a
//! transforming Saga is this sentence.

use super::tokens::Reader;
use crate::effect::{Destination, Effect};
use crate::refs::Ref;
use crate::value::Amount;

/// The name the exiled card is bound to.
pub const EXILED: &str = "exiled";

/// "Exile ~, then return it to the battlefield [transformed] under your control." (or "under its owner's control").
pub fn exile_and_return(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("exile")?;
    r.source().then_some(())?;
    r.punct(',').then_some(())?;
    r.need("then return it to the battlefield")?;
    let transformed = r.word("transformed");
    r.need("under")?;
    let under = if r.words("your control") {
        Some(Ref::You)
    } else {
        r.need("its owner's control")?;
        None
    };
    Some(vec![
        Effect::Exile {
            what: Ref::This,
            bind: Some(EXILED.to_owned()),
            duration: None,
        },
        Effect::Move {
            what: Ref::Bound(EXILED.to_owned()),
            to: Destination::Battlefield {
                tapped: false,
                under,
                transformed,
                counters: Vec::new(),
            },
        },
    ])
}

/// "Exile ~ with three time counters on it.": the card exiles itself and the object it becomes in exile gets the counters
/// (a spell with suspend is then suspended).
pub fn exile_with_counters(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("exile")?;
    r.source().then_some(())?;
    r.need("with")?;
    let (counter, number) = super::counters::counters(r)?;
    r.need("on it")?;
    Some(vec![
        Effect::Exile {
            what: Ref::This,
            bind: Some(EXILED.to_owned()),
            duration: None,
        },
        Effect::PutCounters {
            on: Ref::Bound(EXILED.to_owned()),
            counter,
            count: Amount::Fixed(i32::try_from(number).ok()?),
        },
    ])
}

/// The sentence of the two effects at the start of `effects`, and how many it covers.
pub fn text(effects: &[Effect]) -> Option<(String, usize)> {
    if let [
        Effect::Exile {
            what: Ref::This,
            bind: Some(exiled),
            duration: None,
        },
        Effect::PutCounters {
            on: Ref::Bound(on),
            counter,
            count,
        },
        ..,
    ] = effects
        && exiled == EXILED
        && on == EXILED
    {
        let counters = super::counters::text(counter, count)?;
        return Some((format!("exile ~ with {counters} on it"), 2));
    }
    let [
        Effect::Exile {
            what: Ref::This,
            bind: Some(exiled),
            duration: None,
        },
        Effect::Move {
            what: Ref::Bound(moved),
            to:
                Destination::Battlefield {
                    tapped: false,
                    under,
                    transformed,
                    counters,
                },
        },
        ..,
    ] = effects
    else {
        return None;
    };
    if exiled != EXILED || moved != EXILED || !counters.is_empty() {
        return None;
    }
    let owner = match under {
        Some(Ref::You) => "your",
        None => "its owner's",
        Some(_) => return None,
    };
    Some((
        format!(
            "exile ~, then return it to the battlefield {}under {owner} control",
            if *transformed { "transformed " } else { "" }
        ),
        2,
    ))
}
