//! The top cards revealed and one of them chosen by an opponent: "Reveal the top three cards of your library. An opponent
//! chooses one of them. Put that card into your graveyard and the rest into your hand."

use super::super::amounts::count_word;
use crate::effect::{Destination, Effect};
use crate::refs::{Bounds, Pred, Ref, ZoneKind};
use crate::value::Amount;

/// Where a card goes, as the card words it for the cards of the controller's library.
fn into(to: &Destination) -> Option<&'static str> {
    match to {
        Destination::Hand => Some("your hand"),
        Destination::Graveyard => Some("your graveyard"),
        _ => None,
    }
}

/// The five effects of the reveal, the opponent's choice and the two moves, and their number.
pub(super) fn text(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::LookAtTop {
            who: Ref::You,
            count: Amount::Fixed(count),
            bind: top,
        },
        Effect::Reveal {
            what: Ref::Bound(shown),
        },
        Effect::Choose {
            who: Ref::Opponents,
            from: Ref::Each(pool),
            count: one,
            bind: chosen,
            rest_as: Some(rest),
        },
        Effect::Move {
            what: Ref::Bound(taken),
            to: first,
        },
        Effect::Move {
            what: Ref::Bound(left),
            to: second,
        },
        ..,
    ] = effects
    else {
        return None;
    };
    let cards_looked_at = pool.0 == [Pred::In(ZoneKind::Library), Pred::Is(Ref::Bound(top.clone()))];
    (top == shown && cards_looked_at && *one == Bounds::default() && taken == chosen && left == rest)
        .then_some(())?;
    let words = count_word(u32::try_from(*count).ok()?)?;
    Some((
        format!(
            "reveal the top {words} cards of your library. An opponent chooses one of them. Put that card into {} and the rest into {}",
            into(first)?,
            into(second)?
        ),
        5,
    ))
}
