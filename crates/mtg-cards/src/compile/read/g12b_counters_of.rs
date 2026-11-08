//! "Put its counters on target creature you control": the counters of each kind an object held put on another
//! (`Effect::PutCountersOf`), the object read as it last existed when it has left.

use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::Ref;

/// "put its counters on target creature you control", "put those counters on ~", "put ~'s counters on that creature":
/// "its" and "those" are the card itself where the sentence before names it, else the object of the event.
pub fn g12b_put_of(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("put")?;
    let of = if r.word("its") || r.word("those") {
        match r.it {
            Some(Ref::This) => Ref::This,
            _ => Ref::Triggering,
        }
    } else if r.source() {
        (r.punct('\'') && r.word("s")).then_some(())?;
        Ref::This
    } else {
        return None;
    };
    r.need("counters on")?;
    let on = super::counters::place(r)?;
    Some(vec![Effect::PutCountersOf { on, of, each: None }])
}
