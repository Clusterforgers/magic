//! Another player exiling the top of their own library: "defending player exiles the top twenty cards of their library",
//! "target opponent exiles the top two cards of their library", read from its wording and written back the same way.

use super::library::{top_cards, top_text};
use super::tokens::Reader;
use crate::effect::{EXILED, Effect};
use crate::refs::Ref;

/// "exiles the top twenty cards of their library" after the subject `who`, a player other than you and the one of the
/// event (whose wording is "exile the top card of that player's library").
pub(super) fn exiles_top(r: &mut Reader<'_>, who: &Ref) -> Option<Vec<Effect>> {
    (!matches!(who, Ref::You | Ref::Triggering)).then_some(())?;
    r.need("exiles")?;
    let count = top_cards(r)?;
    r.need("of their library")?;
    Some(vec![Effect::ExileTop {
        who: who.clone(),
        count,
        bind: EXILED.to_owned(),
        sealed: false,
        bottom: false,
    }])
}

/// The sentence of such an exile by `subject` without its first capital and period; none for another effect.
pub(super) fn text(effect: &Effect, subject: &str) -> Option<String> {
    match effect {
        Effect::ExileTop {
            who,
            count,
            sealed: false,
            bottom: false,
            ..
        } if !matches!(who, Ref::You | Ref::Triggering) => Some(format!(
            "{subject} exiles {} of their library",
            top_text(count)?
        )),
        _ => None,
    }
}
