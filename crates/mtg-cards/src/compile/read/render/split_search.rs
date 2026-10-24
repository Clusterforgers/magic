//! A search whose finds go two ways: "Search your library for up to two basic land cards, reveal those cards, put one
//! onto the battlefield tapped and the other into your hand, then shuffle."

use crate::compile::read::library::{destination_text, what_text};
use crate::effect::Effect;
use crate::refs::{Ref, ZoneKind};

/// The sentence for a search, the choice of one find and the move of it and of the rest to two places, then the
/// shuffle; and how many effects it covers.
pub(super) fn text(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::Search {
            who: Ref::You,
            zone: ZoneKind::Library,
            filter,
            count,
            reveal: true,
            bind,
        },
        Effect::Choose {
            who: Ref::You,
            from: Ref::Bound(from),
            count: chosen,
            bind: kept,
            rest_as: Some(rest),
        },
        Effect::Move {
            what: Ref::Bound(first),
            to: first_to,
        },
        Effect::Move {
            what: Ref::Bound(second),
            to: second_to,
        },
        Effect::Shuffle { who: Ref::You },
        ..,
    ] = effects
    else {
        return None;
    };
    (from == bind && chosen.is_one() && first == kept && second == rest).then_some(())?;
    (!count.is_one()).then_some(())?;
    Some((
        format!(
            "search your library for {}, reveal those cards, put one {} and the other {}, then shuffle",
            what_text(filter, count)?,
            destination_text(first_to)?,
            destination_text(second_to)?
        ),
        5,
    ))
}
