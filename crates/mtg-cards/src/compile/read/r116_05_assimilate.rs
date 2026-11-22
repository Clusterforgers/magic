//! Assimilate of a card the player chooses: "assimilate a creature card exiled with it" reads as a choice of one such
//! card and the assimilate of the card chosen, and writes back.

use super::selectors::{Form, phrase_in, phrase_text};
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::{Bounds, Pred, Ref, Selector, ZoneKind};

/// The name the chosen card is bound to.
const R116_CHOSEN: &str = "assimilated";

/// The cards exiled with the source.
fn r116_exiled_with() -> [Pred; 2] {
    [Pred::In(ZoneKind::Exile), Pred::ExiledWith(Ref::This)]
}

/// "assimilate a creature card exiled with it": the player chooses one card exiled with the source, which is then
/// assimilated; with none there, nothing happens.
pub(super) fn r116_assimilate_chosen(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("assimilate")?;
    if !(r.word("a") || r.word("an")) {
        return None;
    }
    let mut selector = phrase_in(r, Form::Subject, true)?;
    if r.last_word() != Some("card") {
        return None;
    }
    r.need("exiled with")?;
    if !(r.word("it") || r.source()) {
        return None;
    }
    if selector.0.iter().any(|pred| matches!(pred, Pred::In(_))) {
        return None;
    }
    selector.0.extend(r116_exiled_with());
    Some(vec![
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(selector),
            count: Bounds::default(),
            bind: R116_CHOSEN.to_owned(),
            rest_as: None,
            differ: None,
        },
        Effect::R116Assimilate {
            what: Ref::Bound(R116_CHOSEN.to_owned()),
        },
    ])
}

/// Writes back the choice and the assimilate of the card chosen: "assimilate a creature card exiled with it".
pub(super) fn r116_assimilate_chosen_text(choose: &Effect, assimilated: &Effect) -> Option<String> {
    let (
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(selector),
            count,
            bind,
            rest_as: None,
            differ: None,
        },
        Effect::R116Assimilate {
            what: Ref::Bound(name),
        },
    ) = (choose, assimilated)
    else {
        return None;
    };
    let tail = r116_exiled_with();
    let at = selector.0.len().checked_sub(tail.len())?;
    if bind != name || !count.is_one() || selector.0[at..] != tail {
        return None;
    }
    let noun = phrase_text(&Selector(selector.0[..at].to_vec()), Form::Subject)?;
    let article = if noun.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    Some(format!("assimilate {article} {noun} card exiled with it"))
}
