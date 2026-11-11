//! The sticker wording (CR 123): "put a sticker on a nonland permanent you own", read into `Effect::PutSticker` and
//! written back the same way.

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector};

/// Whether the objects are ones the player owns: a sticker goes only on a permanent its player owns (CR 123).
fn owned(on: &Selector) -> bool {
    on.0.contains(&Pred::OwnedBy(Ref::You))
}

/// "put a sticker on a nonland permanent you own": the objects after "on a", which the player owns.
pub(super) fn un34_put_sticker(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("put a sticker on")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let mut on = phrase(r, Form::Subject)?;
    r.need("you own")?;
    (!owned(&on)).then_some(())?;
    on.0.push(Pred::OwnedBy(Ref::You));
    Some(vec![Effect::PutSticker { who: Ref::You, on }])
}

/// What `un34_put_sticker` read, written back for "you".
pub(super) fn un34_put_text(who: &Ref, on: &Selector) -> Option<String> {
    if *who != Ref::You || !owned(on) {
        return None;
    }
    let rest = Selector(
        on.0.iter()
            .filter(|pred| **pred != Pred::OwnedBy(Ref::You))
            .cloned()
            .collect(),
    );
    let objects = phrase_text(&rest, Form::Subject)?;
    Some(format!(
        "put a sticker on {} {objects} you own",
        super::triggers::article(&objects)
    ))
}
